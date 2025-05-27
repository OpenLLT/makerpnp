use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// A file picker
/// * prevents multiple concurrent pick operations
/// * provides an API convenient for UI usage (see `is_picking` and `picked`)
///
/// Currently only picks files, but the API could be expanded.
#[derive(Default, Debug)]
pub struct Picker {
    state: PickerState,
}

#[derive(Default, Debug)]
enum PickerState {
    #[default]
    Pending,
    // use a boolean to indicate of picking has completed
    Picking(Arc<Mutex<(bool, Option<Vec<PathBuf>>)>>),
}

#[derive(Clone, PartialEq)]
pub enum PickError {
    NotPicking,
    InProgress,
    Cancelled,
}

impl Picker {
    pub fn is_picking(&self) -> bool {
        matches!(self.state, PickerState::Picking(_))
    }

    fn prepare(&mut self, some_fn: fn() -> Option<Vec<PathBuf>>) {
        // initialise the boolean flag in the mutex to false, so that when the main thread continues it can see a
        // file has not been picked yet.  note that the mutex may not be locked until the picker thread starts to run
        // and lock it.
        let picker = Arc::new(Mutex::new((false, None)));
        self.state = PickerState::Picking(picker.clone());
        std::thread::Builder::new()
            .name("picker".to_owned())
            .spawn(move || {
                let mut guard = picker.lock().unwrap();
                *guard = (true, some_fn());
            })
            .unwrap();
    }

    pub fn pick_file(&mut self) {
        self.prepare(|| {
            rfd::FileDialog::new()
                .pick_file()
                .map(|file| vec![file])
        });
    }

    pub fn pick_files(&mut self) {
        self.prepare(|| rfd::FileDialog::new().pick_files());
    }

    pub fn pick_folder(&mut self) {
        self.prepare(|| {
            rfd::FileDialog::new()
                .pick_folder()
                .map(|folder| vec![folder])
        });
    }

    /// when picked, returns the picked paths, or an error indicating the reason
    ///
    /// See also [`Self::picked`]
    ///
    /// this method is designed to be very fast while the picker is not picking (pending)
    pub fn picked_multi(&mut self) -> Result<Vec<PathBuf>, PickError> {
        let mut was_picked = false;

        let result = match &mut self.state {
            PickerState::Picking(arc) => {
                if let Ok(mut guard) = arc.try_lock() {
                    match &mut *guard {
                        (true, picked) => {
                            was_picked = true;
                            match picked.take() {
                                Some(picked_path) => Ok(picked_path),
                                None => Err(PickError::Cancelled),
                            }
                        }
                        // arc not locked, but not picked yet either
                        (false, _) => Err(PickError::InProgress),
                    }
                } else {
                    // not picked yet, arc locked by thread
                    Err(PickError::InProgress)
                }
            }
            PickerState::Pending =>
            // not picked
            {
                Err(PickError::NotPicking)
            }
        };

        if was_picked {
            // this causes the arc and mutex to be dropped, ready for the next pick.
            self.state = PickerState::Pending;
        }

        result
    }

    /// when picked, returns the first picked path, or an error indicating the reason
    ///
    /// use when picking a single file or folder, otherwise use [`Self::picked_multi`]
    pub fn picked(&mut self) -> Result<PathBuf, PickError> {
        self.picked_multi()
            .map(|paths| paths.first().unwrap().clone())
    }
}
