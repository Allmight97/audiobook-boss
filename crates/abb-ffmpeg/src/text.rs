use std::ffi::{c_char, CStr, CString};
use std::ptr;

use ffmpeg_sys_next as sys;
use zeroize::Zeroizing;

use crate::error::{Error, Result, Step};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text(CString);

impl Text {
    pub fn new(value: &str, step: Step) -> Result<Self> {
        CString::new(value)
            .map(Self)
            .map_err(|_| Error::NulInText { step })
    }

    pub fn from_bytes(value: &[u8], step: Step) -> Result<Self> {
        CString::new(value)
            .map(Self)
            .map_err(|_| Error::NulInText { step })
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn display(&self) -> String {
        match self.0.to_str() {
            Ok(text) => text.to_owned(),
            Err(_) => self
                .0
                .as_bytes()
                .iter()
                .map(|&byte| char::from(byte))
                .collect(),
        }
    }

    pub(crate) fn as_ptr(&self) -> *const c_char {
        self.0.as_ptr()
    }

    pub(crate) fn as_c_str(&self) -> &CStr {
        &self.0
    }

    pub(crate) fn copied_from(text: &CStr) -> Self {
        Self(text.to_owned())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags(Vec<(Text, Text)>);

impl Tags {
    pub fn get(&self, key: &str) -> Option<&Text> {
        self.0
            .iter()
            .find(|(name, _)| name.as_bytes().eq_ignore_ascii_case(key.as_bytes()))
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

    pub(crate) fn read(dictionary: Option<&sys::AVDictionary>) -> Self {
        let Some(dictionary) = dictionary else {
            return Self(Vec::new());
        };
        let mut tags = Vec::new();
        let mut entry = ptr::null();
        loop {
            // SAFETY: `dictionary` is live and unchanged during this loop; `entry`
            // is null or the previous entry returned by FFmpeg.
            entry = unsafe { sys::av_dict_iterate(dictionary, entry) };
            if entry.is_null() {
                return Self(tags);
            }
            // SAFETY: a non-null entry has NUL-terminated key and value strings.
            let (key, value) =
                unsafe { (CStr::from_ptr((*entry).key), CStr::from_ptr((*entry).value)) };
            tags.push((Text::copied_from(key), Text::copied_from(value)));
        }
    }

    pub(crate) fn to_dictionary(&self, step: Step) -> Result<Dictionary> {
        let mut dictionary = Dictionary::default();
        for (key, value) in &self.0 {
            dictionary.set(key.as_c_str(), value.as_c_str(), step)?;
        }
        Ok(dictionary)
    }
}

pub(crate) struct Dictionary(*mut sys::AVDictionary);

impl Default for Dictionary {
    fn default() -> Self {
        Self(ptr::null_mut())
    }
}

impl Dictionary {
    pub(crate) fn set(&mut self, key: &CStr, value: &CStr, step: Step) -> Result<()> {
        // SAFETY: `self.0` is null or an owned dictionary; FFmpeg copies both
        // NUL-terminated strings.
        let code = unsafe { sys::av_dict_set(&mut self.0, key.as_ptr(), value.as_ptr(), 0) };
        Error::check(step, code).map(drop)
    }

    pub(crate) fn as_mut_slot(&mut self) -> *mut *mut sys::AVDictionary {
        &mut self.0
    }

    pub(crate) fn keys(&self) -> Vec<String> {
        // SAFETY: `self.0` is null or an owned live dictionary.
        Tags::read(unsafe { self.0.as_ref() })
            .0
            .into_iter()
            .map(|(key, _)| key.display())
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

#[derive(Default)]
pub struct SecretOptions(Vec<(CString, Zeroizing<Vec<u8>>)>);

impl SecretOptions {
    pub fn push(&mut self, name: &str, value: &str) -> Result<()> {
        let step = Step::OpenInput;
        let name = CString::new(name).map_err(|_| Error::NulInText { step })?;
        if value.as_bytes().contains(&0) {
            return Err(Error::NulInText { step });
        }
        let mut stored = Zeroizing::new(Vec::with_capacity(value.len() + 1));
        stored.extend_from_slice(value.as_bytes());
        stored.push(0);
        self.0.push((name, stored));
        Ok(())
    }

    pub(crate) fn to_dictionary(&self) -> Result<Dictionary> {
        let step = Step::OpenInput;
        let mut dictionary = Dictionary::default();
        for (name, value) in &self.0 {
            let value = CStr::from_bytes_with_nul(value).map_err(|_| Error::NulInText { step })?;
            dictionary.set(name, value, step)?;
        }
        Ok(dictionary)
    }
}
