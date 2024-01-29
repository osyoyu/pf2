use std::collections::HashMap;
use std::ffi::{c_char, CStr};
use std::hash::Hasher;

use rb_sys::*;

use crate::backtrace::Backtrace;
use crate::profile::Profile;

#[derive(Debug, Deserialize, Serialize)]
pub struct ProfileSerializer {
    threads: HashMap<ThreadId, ThreadProfile>,
}

type ThreadId = VALUE;

#[derive(Debug, Deserialize, Serialize)]
struct ThreadProfile {
    thread_id: ThreadId,
    stack_tree: StackTreeNode,
    #[serde(rename = "frames")]
    frame_table: HashMap<FrameTableId, FrameTableEntry>,
    samples: Vec<ProfileSample>,
}

impl ThreadProfile {
    fn new(thread_id: ThreadId) -> ThreadProfile {
        ThreadProfile {
            thread_id,
            // The root node
            stack_tree: StackTreeNode {
                children: HashMap::new(),
                node_id: 0,
                frame_id: 0,
            },
            frame_table: HashMap::new(),
            samples: vec![],
        }
    }
}

type StackTreeNodeId = i32;

// Arbitary value which is used inside StackTreeNode.
// This VALUE should not be dereferenced as a pointer; we're merely using its pointer as a unique value.
// (Probably should be reconsidered)
type FrameTableId = VALUE;

#[derive(Debug, Deserialize, Serialize)]
struct StackTreeNode {
    // TODO: Maybe a Vec<StackTreeNode> is enough?
    // There's no particular meaning in using FrameTableId as key
    children: HashMap<FrameTableId, StackTreeNode>,
    // An arbitary ID (no particular meaning)
    node_id: StackTreeNodeId,
    // ?
    frame_id: FrameTableId,
}

#[derive(Debug, Deserialize, Serialize)]
struct FrameTableEntry {
    id: FrameTableId,
    entry_type: FrameTableEntryType,
    full_label: String,
}

#[derive(Debug, Deserialize, Serialize)]
enum FrameTableEntryType {
    Ruby,
    Native,
}

// Represents leaf (末端)
#[derive(Debug, Deserialize, Serialize)]
struct ProfileSample {
    elapsed_ns: u128,
    stack_tree_id: StackTreeNodeId,
}

impl ProfileSerializer {
    pub fn serialize(profile: &Profile) -> String {
        let mut sequence = 1;

        let mut serializer = ProfileSerializer {
            threads: HashMap::new(),
        };

        unsafe {
            // Process each sample
            for sample in profile.samples.iter() {
                let mut merged_stack: Vec<FrameTableEntry> = vec![];

                // Process C-level stack

                // A vec to keep the "programmer's" C stack trace.
                // A single PC may be mapped to multiple inlined frames,
                // so we keep the expanded stack frame in this Vec.
                let mut c_stack: Vec<String> = vec![];
                for i in 0..sample.c_backtrace_pcs[0] {
                    let pc = sample.c_backtrace_pcs[i + 1];
                    Backtrace::backtrace_syminfo(
                        &profile.backtrace_state,
                        pc,
                        |_pc: usize, symname: *const c_char, _symval: usize, _symsize: usize| {
                            if symname.is_null() {
                                c_stack.push("(no symbol information)".to_owned());
                            } else {
                                c_stack.push(CStr::from_ptr(symname).to_str().unwrap().to_owned());
                            }
                        },
                        Some(Backtrace::backtrace_error_callback),
                    );
                }

                // Strip the C stack trace:
                // - Remove Pf2-related frames which are always captured
                // - Remove frames below rb_vm_exec
                let mut reached_ruby = false;
                c_stack.retain(|frame| {
                    if reached_ruby {
                        return false;
                    }
                    if frame.contains("pf2") {
                        return false;
                    }
                    if frame.contains("rb_vm_exec") || frame.contains("vm_call_cfunc_with_frame") {
                        reached_ruby = true;
                        return false;
                    }
                    true
                });

                for frame in c_stack.iter() {
                    merged_stack.push(FrameTableEntry {
                        id: calculate_id_for_c_frame(frame),
                        entry_type: FrameTableEntryType::Native,
                        full_label: frame.to_string(),
                    });
                }

                // Process Ruby-level stack

                let ruby_stack_depth = sample.line_count;
                for i in 0..ruby_stack_depth {
                    let frame: VALUE = sample.frames[i as usize];
                    merged_stack.push(FrameTableEntry {
                        id: frame,
                        entry_type: FrameTableEntryType::Ruby,
                        full_label: CStr::from_ptr(rb_string_value_cstr(
                            &mut rb_profile_frame_full_label(frame),
                        ))
                        .to_str()
                        .unwrap()
                        .to_owned(),
                    });
                }

                // Find the Thread profile for this sample
                let thread_serializer = serializer
                    .threads
                    .entry(sample.ruby_thread)
                    .or_insert(ThreadProfile::new(sample.ruby_thread));

                // Stack frames, shallow to deep
                let mut stack_tree = &mut thread_serializer.stack_tree;

                while let Some(frame_table_entry) = merged_stack.pop() {
                    stack_tree = stack_tree.children.entry(frame_table_entry.id).or_insert({
                        let node = StackTreeNode {
                            children: HashMap::new(),
                            node_id: sequence,
                            frame_id: frame_table_entry.id,
                        };
                        sequence += 1;
                        node
                    });

                    if merged_stack.is_empty() {
                        // This is the leaf node, record a Sample
                        let elapsed_ns = (sample.timestamp - profile.start_timestamp).as_nanos();
                        thread_serializer.samples.push(ProfileSample {
                            elapsed_ns,
                            stack_tree_id: stack_tree.node_id,
                        });
                    }

                    // Register frame metadata to frame table, if not registered yet
                    thread_serializer
                        .frame_table
                        .entry(frame_table_entry.id)
                        .or_insert(frame_table_entry);
                }
            }
        }

        serde_json::to_string(&serializer).unwrap()
    }
}

fn calculate_id_for_c_frame<T: std::hash::Hash>(t: &T) -> FrameTableId {
    let mut s = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}
