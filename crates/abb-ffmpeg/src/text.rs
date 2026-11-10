use std::ffi::{c_char, CStr, CString};
use std::ptr;

use ffmpeg_sys_next as sys;

use crate::error::{Error, Result, Step};

/// A text value as FFmpeg stores it: bytes without NUL, not always UTF-8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text(CString);

impl Text {
    pub fn new(value: &str, step: Step) -> Result<Self> {
        CString::new(value)
            .map(Self)
            .map_err(|_| Error::NulInText { step })
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn to_string_lossy(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }

    pub(crate) fn as_ptr(&self) -> *const c_char {
        self.0.as_ptr()
    }

    /// # Safety
    /// `ptr` is a valid NUL-terminated string.
    pub(crate) unsafe fn copied_from(ptr: *const c_char) -> Self {
        // SAFETY: the caller guarantees a valid NUL-terminated string.
        Self(unsafe { CStr::from_ptr(ptr) }.to_owned())
    }
}

/// Tags in FFmpeg's order, copied out of a container, stream, or chapter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags(Vec<(Text, Text)>);

impl Tags {
    pub fn get(&self, key: &str) -> Option<&Text> {
        self.0
            .iter()
            .find(|(name, _)| name.as_bytes() == key.as_bytes())
            .map(|(_, value)| value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Text, &Text)> {
        self.0.iter().map(|(key, value)| (key, value))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn insert(&mut self, key: Text, value: Text) {
        self.0.push((key, value));
    }

    pub fn push(&mut self, key: &str, value: &str, step: Step) -> Result<()> {
        self.0
            .push((Text::new(key, step)?, Text::new(value, step)?));
        Ok(())
    }

    pub(crate) fn read(dictionary: *const sys::AVDictionary) -> Self {
        let mut tags = Vec::new();
        let mut entry = ptr::null();
        loop {
            // SAFETY: `dictionary` is null or a live dictionary its owner keeps
            // unchanged during this loop; `entry` is null or its previous entry.
            entry = unsafe { sys::av_dict_iterate(dictionary, entry) };
            if entry.is_null() {
                return Self(tags);
            }
            // SAFETY: a non-null entry has NUL-terminated key and value strings.
            tags.push(unsafe {
                (
                    Text::copied_from((*entry).key),
                    Text::copied_from((*entry).value),
                )
            });
        }
    }

    pub(crate) fn to_dictionary(&self, step: Step) -> Result<Dictionary> {
        let mut dictionary = Dictionary::default();
        for (key, value) in &self.0 {
            dictionary.set(key.as_ptr(), value.as_ptr(), step)?;
        }
        Ok(dictionary)
    }
}

/// An owned `AVDictionary`, freed on drop.
pub(crate) struct Dictionary(*mut sys::AVDictionary);

impl Default for Dictionary {
    fn default() -> Self {
        Self(ptr::null_mut())
    }
}

impl Dictionary {
    pub(crate) fn set(
        &mut self,
        key: *const c_char,
        value: *const c_char,
        step: Step,
    ) -> Result<()> {
        // SAFETY: `self.0` is null or an owned dictionary; FFmpeg copies both
        // NUL-terminated strings the caller passes.
        let code = unsafe { sys::av_dict_set(&mut self.0, key, value, 0) };
        Error::check(step, code).map(drop)
    }

    pub(crate) fn as_mut_slot(&mut self) -> *mut *mut sys::AVDictionary {
        &mut self.0
    }

    pub(crate) fn keys(&self) -> Vec<String> {
        Tags::read(self.0)
            .0
            .into_iter()
            .map(|(key, _)| key.to_string_lossy())
            .collect()
    }

    pub(crate) fn into_raw(mut self) -> *mut sys::AVDictionary {
        std::mem::replace(&mut self.0, ptr::null_mut())
    }
}

impl Drop for Dictionary {
    fn drop(&mut self) {
        // SAFETY: `self.0` is null or a dictionary this value owns.
        unsafe { sys::av_dict_free(&mut self.0) };
    }
}

/// Input options that hold secrets, such as a decrypt key. It has no `Debug`
/// or `Clone`, and it zeroes its values when dropped. FFmpeg keeps its own
/// copy while the input is open.
#[derive(Default)]
pub struct SecretOptions(Vec<(CString, Vec<u8>)>);

impl SecretOptions {
    pub fn push(&mut self, name: &str, value: &str) -> Result<()> {
        let step = Step::OpenInput;
        let name = CString::new(name).map_err(|_| Error::NulInText { step })?;
        if value.as_bytes().contains(&0) {
            return Err(Error::NulInText { step });
        }
        let mut stored = Vec::with_capacity(value.len() + 1);
        stored.extend_from_slice(value.as_bytes());
        stored.push(0);
        self.0.push((name, stored));
        Ok(())
    }

    pub(crate) fn to_dictionary(&self) -> Result<Dictionary> {
        let mut dictionary = Dictionary::default();
        for (name, value) in &self.0 {
            dictionary.set(name.as_ptr(), value.as_ptr().cast(), Step::OpenInput)?;
        }
        Ok(dictionary)
    }
}

impl Drop for SecretOptions {
    fn drop(&mut self) {
        for (_, value) in &mut self.0 {
            for byte in value.iter_mut() {
                // SAFETY: `byte` is a valid exclusive reference into the vector.
                unsafe { ptr::write_volatile(byte, 0) };
            }
        }
    }
}
