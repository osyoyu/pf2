use std::{collections::HashMap, ffi::CStr};

use rb_sys::*;

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
    full_label: String,
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
                // Find the Thread profile for this sample
                let thread_serializer = serializer
                    .threads
                    .entry(sample.ruby_thread)
                    .or_insert(ThreadProfile::new(sample.ruby_thread));

                // Stack frames, shallow to deep
                let mut stack_tree = &mut thread_serializer.stack_tree;

                for i in (0..(sample.line_count - 1)).rev() {
                    let frame = sample.frames[i as usize];

                    // Register frame metadata to frame table, if not registered yet
                    let frame_table_id: FrameTableId = frame;
                    thread_serializer
                        .frame_table
                        .entry(frame_table_id)
                        .or_insert(FrameTableEntry {
                            full_label: CStr::from_ptr(rb_string_value_cstr(
                                &mut rb_profile_frame_full_label(frame),
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

                    if i == 0 {
                        // This is the leaf node, record a Sample
                        let elapsed_ns = (sample.timestamp - profile.start_timestamp).as_nanos();
                        thread_serializer.samples.push(ProfileSample {
                            elapsed_ns,
                            stack_tree_id: stack_tree.node_id,
                        });
                    }
                }
            }
        }

        serde_json::to_string(&serializer).unwrap()
    }
}
