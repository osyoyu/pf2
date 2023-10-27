use std::{collections::HashMap, ffi::CStr};

use rb_sys::*;

#[derive(Debug, Deserialize, Serialize)]
pub struct Profile {
    threads: HashMap<ThreadId, ThreadProfile>,
}

// The native thread ID which can be obtained through `Thread#native_thread_id`.
// May change when MaNy (M:N threads) get stablized.
type ThreadId = i64;

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
    full_label: String,
}

// Represents leaf (末端)
#[derive(Debug, Deserialize, Serialize)]
struct ProfileSample {
    elapsed_ns: u128,
    stack_tree_id: StackTreeNodeId,
}

impl Profile {
    // Build a profile from collected samples
    // Should be called in a Ruby thread which has acquired the GVL
    pub fn from_samples(samples: &[crate::sample_collector::Sample]) -> Profile {
        let mut sequence = 1;

        let mut profile = Profile {
            threads: HashMap::new(),
        };

        unsafe {
            // Process each sample
            for sample in samples.iter() {
                // Find the Thread profile for this sample
                let thread_profile = profile
                    .threads
                    .entry(sample.ruby_thread_native_thread_id)
                    .or_insert(ThreadProfile::new(sample.ruby_thread_native_thread_id));

                // Stack frames, shallow to deep
                let mut stack_tree = &mut thread_profile.stack_tree;

                let mut it = sample.frames.iter().rev().peekable();
                while let Some(frame) = it.next() {
                    // Register frame metadata to frame table, if not registered yet
                    let frame_table_id: FrameTableId = *frame;
                    thread_profile
                        .frame_table
                        .entry(frame_table_id)
                        .or_insert(FrameTableEntry {
                            full_label: CStr::from_ptr(rb_string_value_cstr(
                                &mut rb_profile_frame_full_label(*frame),
                            ))
                            .to_str()
                            .unwrap()
                            .to_string(),
                        });

                    stack_tree = stack_tree.children.entry(frame_table_id).or_insert({
                        let node = StackTreeNode {
                            children: HashMap::new(),
                            node_id: sequence,
                            frame_id: frame_table_id,
                        };
                        sequence += 1;
                        node
                    });

                    if it.peek().is_none() {
                        // This is the leaf node, record a Sample
                        thread_profile.samples.push(ProfileSample {
                            elapsed_ns: sample.elapsed_ns,
                            stack_tree_id: stack_tree.node_id,
                        });
                    }
                }
            }
        }

        profile
    }
}
