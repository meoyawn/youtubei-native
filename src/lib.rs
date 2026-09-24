//! Safe, owning wrapper over the compiled youtubei.js C units.
mod ffi;
mod host;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cell::RefCell;
use std::ffi::CStr;
use std::fmt;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard};

static RUNTIME_OWNER: Mutex<()> = Mutex::new(());
const MAX_RESULT: usize = 8 * 1024 * 1024;

#[derive(Debug)]
pub enum Error {
    Busy,
    Initialization(String),
    Javascript(String),
    InvalidInput(String),
    InvalidOutput(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => write!(f, "A compiled YouTube runtime is already open"),
            Self::Initialization(message)
            | Self::Javascript(message)
            | Self::InvalidInput(message)
            | Self::InvalidOutput(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub duration: u64,
    pub published: String,
    pub available: bool,
}

/// Owns the runtime and its native units. All calls stay on the creating thread.
/// The experiment permits one live owner, following SHUnit's ownership contract.
pub struct Youtube {
    runtime: NonNull<ffi::Runtime>,
    _owner: MutexGuard<'static, ()>,
    _thread: PhantomData<Rc<()>>,
}

impl Youtube {
    pub fn new() -> Result<Self> {
        let owner = RUNTIME_OWNER.try_lock().map_err(|_| Error::Busy)?;
        let mut arguments = [
            c"youtubei-native".as_ptr().cast_mut(),
            c"-Xhermes-internal-test-methods".as_ptr().cast_mut(),
            c"-gc-init-heap=8M".as_ptr().cast_mut(),
        ];
        let mut error = std::ptr::null_mut();
        // SAFETY: argv is valid for this call; Hermes only reads its strings.
        let pointer = unsafe {
            ffi::_sh_init_with_error(arguments.len() as i32, arguments.as_mut_ptr(), &mut error)
        };
        let Some(runtime) = NonNull::new(pointer) else {
            let message = if error.is_null() {
                "Hermes initialization failed".to_owned()
            } else {
                // SAFETY: Hermes returns a strdup-owned, NUL-terminated error.
                let message = unsafe { CStr::from_ptr(error) }
                    .to_string_lossy()
                    .into_owned();
                unsafe { ffi::free(error.cast()) };
                message
            };
            return Err(Error::Initialization(message));
        };
        let mut youtube = Self {
            runtime,
            _owner: owner,
            _thread: PhantomData,
        };
        youtube.initialize(ffi::sh_export_youtubei_host)?;
        youtube.initialize(ffi::sh_export_youtubei_youtube)?;
        Ok(youtube)
    }

    pub fn parse_text(&mut self, value: &Value) -> Result<String> {
        self.call("parseText", &value.to_string())
    }

    pub fn session_client_name(&mut self) -> Result<String> {
        self.call("sessionName", "")
    }

    pub fn playlist(&mut self, id: &str) -> Result<Vec<Video>> {
        if id.is_empty() {
            return Err(Error::InvalidInput("Playlist ID is empty".into()));
        }
        let result = self.call("playlistPages", &serde_json::json!({"id": id}).to_string())?;
        serde_json::from_str(&result).map_err(|error| Error::InvalidOutput(error.to_string()))
    }

    fn initialize(&mut self, creator: ffi::UnitCreator) -> Result<()> {
        // SAFETY: this object exclusively owns a live runtime. The creator
        // allocates a unit that Hermes owns until _sh_done. This guarded C API
        // catches native JS exceptions before returning across the Rust frame.
        let ok = unsafe { ffi::_sh_initialize_units(self.runtime.as_ptr(), 1, creator) };
        if ok {
            Ok(())
        } else {
            Err(Error::Initialization(
                "Native unit failed; see stderr for its exception".into(),
            ))
        }
    }

    fn call(&mut self, method: &'static str, input: &str) -> Result<String> {
        if input.len() > i32::MAX as usize {
            return Err(Error::InvalidInput("Input exceeds native ABI limit".into()));
        }
        INVOCATION.with(|state| {
            *state.borrow_mut() = Invocation {
                method: method.as_bytes(),
                input: input.as_bytes().to_vec(),
                ..Invocation::default()
            }
        });
        let initialized = self.initialize(ffi::sh_export_youtubei_driver);
        let result = INVOCATION.with(|state| std::mem::take(&mut *state.borrow_mut()));
        initialized?;
        if result.length != Some(result.output.len()) {
            return Err(Error::InvalidOutput(
                "Native result was incomplete or exceeded 8 MiB".into(),
            ));
        }
        let output = String::from_utf8(result.output)
            .map_err(|error| Error::InvalidOutput(error.to_string()))?;
        if result.status == Some(0) {
            Ok(output)
        } else {
            Err(Error::Javascript(output))
        }
    }
}

impl Drop for Youtube {
    fn drop(&mut self) {
        // SAFETY: this is the sole owner, calls have completed, and the runtime
        // is destroyed on the thread that created it before releasing the guard.
        unsafe { ffi::_sh_done(self.runtime.as_ptr()) };
        host::reset();
    }
}

#[derive(Default)]
struct Invocation {
    method: &'static [u8],
    input: Vec<u8>,
    output: Vec<u8>,
    length: Option<usize>,
    status: Option<i32>,
}
thread_local! { static INVOCATION: RefCell<Invocation> = RefCell::new(Invocation::default()); }

#[unsafe(no_mangle)]
extern "C" fn youtubei_input_length(method: i32) -> i32 {
    INVOCATION.with(|state| {
        let state = state.borrow();
        if method == 1 {
            state.method.len() as i32
        } else {
            state.input.len() as i32
        }
    })
}

#[unsafe(no_mangle)]
extern "C" fn youtubei_input_byte(method: i32, index: i32) -> i32 {
    INVOCATION.with(|state| {
        let state = state.borrow();
        let bytes = if method == 1 {
            state.method
        } else {
            &state.input
        };
        bytes.get(index as usize).copied().unwrap_or(0).into()
    })
}

#[unsafe(no_mangle)]
extern "C" fn youtubei_output_byte(index: i32, byte: i32) {
    INVOCATION.with(|state| {
        let mut state = state.borrow_mut();
        if index >= 0 && index as usize == state.output.len() && state.output.len() < MAX_RESULT {
            state.output.push(byte as u8);
        }
    });
}

#[unsafe(no_mangle)]
extern "C" fn youtubei_complete(status: i32, length: i32) {
    INVOCATION.with(|state| {
        let mut state = state.borrow_mut();
        state.status = Some(status);
        state.length = usize::try_from(length).ok();
    });
}

#[cfg(test)]
mod tests;
