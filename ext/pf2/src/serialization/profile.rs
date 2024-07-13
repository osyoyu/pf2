#[derive(Clone, Deserialize, Serialize)]
pub struct Profile {
    pub samples: Vec<Sample>,
    pub locations: Vec<Location>,
    pub functions: Vec<Function>,
    pub start_timestamp_ns: u128,
    pub duration_ns: u128,
}

pub type LocationIndex = usize;
pub type FunctionIndex = usize;

/// Sample
#[derive(Clone, Serialize, Deserialize)]
pub struct Sample {
    /// The stack leading to this sample.
    /// The leaf node will be stored at `stack[0]`.
    pub stack: Vec<LocationIndex>,
    pub ruby_thread_id: Option<u64>,
}

/// Location represents a location (line) in the source code when a sample was captured.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub function_index: FunctionIndex,
    pub lineno: i32,
    pub address: Option<usize>,
}

/// Function represents a Ruby method or a C function in the profile.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Function {
    pub implementation: FunctionImplementation,
    pub name: Option<String>, // unique key
    pub filename: Option<String>,
    /// The first line number in the method/function definition.
    /// For the actual location (line) which was hit during sample capture, refer to `Location.lineno`.
    pub start_lineno: Option<i32>,
    pub start_address: Option<usize>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum FunctionImplementation {
    Ruby,
    C,
}
