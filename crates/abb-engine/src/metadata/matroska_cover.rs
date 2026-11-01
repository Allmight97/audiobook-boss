use super::embedded_cover::{cover_error, read_exact_at, MAX_CONTAINER_RECORDS};
use super::thumbnail::THUMBNAIL_MAX_ENCODED_BYTES;
use crate::errors::Result;
use std::fs::File;

pub(super) fn is_cover(name: &str, mime: &str) -> bool {
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    matches!(
        stem,
        "cover" | "small_cover" | "cover_land" | "small_cover_land"
    ) && matches!(extension, "jpg" | "jpeg" | "png")
        && matches!(mime, "image/jpeg" | "image/png")
}

struct Element {
    id: u64,
    start: u64,
    end: u64,
}

struct Scan<'a> {
    file: &'a mut File,
    records: usize,
}

impl Scan<'_> {
    fn vint(&mut self, offset: &mut u64, end: u64, id: bool) -> Result<(u64, bool)> {
        let mut bytes = [0u8; 8];
        read_exact_at(self.file, *offset, &mut bytes[..1], end)?;
        let width = bytes[0].leading_zeros() as usize + 1;
        if width > if id { 4 } else { 8 } {
            return Err(cover_error("Invalid Matroska element header"));
        }
        read_exact_at(self.file, *offset, &mut bytes[..width], end)?;
        let mut value = u64::from(if id {
            bytes[0]
        } else {
            bytes[0] & (0xffu64 >> width) as u8
        });
        for byte in &bytes[1..width] {
            value = (value << 8) | u64::from(*byte);
        }
        *offset += width as u64;
        Ok((value, !id && value == (1u64 << (7 * width)) - 1))
    }

    fn next(&mut self, offset: &mut u64, end: u64) -> Result<Option<Element>> {
        if *offset == end {
            return Ok(None);
        }
        self.records += 1;
        if self.records > MAX_CONTAINER_RECORDS {
            return Err(cover_error(
                "Matroska cover scan exceeded the element limit",
            ));
        }
        let (id, _) = self.vint(offset, end, true)?;
        let (size, unknown) = self.vint(offset, end, false)?;
        let start = *offset;
        // A streaming Segment can extend to EOF. Other unknown-size elements
        // cannot be skipped safely by this bounded metadata reader.
        let stop = if unknown && id == 0x18538067 {
            end
        } else if unknown {
            return Err(cover_error(
                "Cannot bound an unknown-size Matroska cover element",
            ));
        } else {
            start
                .checked_add(size)
                .filter(|stop| *stop <= end)
                .ok_or_else(|| cover_error("Matroska element exceeds its container"))?
        };
        *offset = stop;
        Ok(Some(Element {
            id,
            start,
            end: stop,
        }))
    }

    fn text(&mut self, element: &Element) -> Result<String> {
        let size = element.end - element.start;
        if size > 256 {
            return Ok(String::new());
        }
        let mut bytes = vec![0; size as usize];
        read_exact_at(self.file, element.start, &mut bytes, element.end)?;
        Ok(String::from_utf8_lossy(&bytes)
            .trim_end_matches('\0')
            .to_owned())
    }

    fn attached_file(&mut self, parent: Element) -> Result<Option<Vec<u8>>> {
        let mut offset = parent.start;
        let mut name = String::new();
        let mut mime = String::new();
        let mut data = None;
        while let Some(element) = self.next(&mut offset, parent.end)? {
            match element.id {
                0x466e => name = self.text(&element)?,
                0x4660 => mime = self.text(&element)?,
                0x465c => data = Some(element),
                _ => {}
            }
        }
        if !is_cover(&name, &mime) {
            return Ok(None);
        }
        let Some(data) = data else { return Ok(None) };
        let size = data.end - data.start;
        if size > THUMBNAIL_MAX_ENCODED_BYTES as u64 {
            return Err(cover_error(
                "Matroska cover exceeds the thumbnail input limit",
            ));
        }
        let mut bytes = vec![0; size as usize];
        read_exact_at(self.file, data.start, &mut bytes, data.end)?;
        Ok(Some(bytes))
    }

    fn attachments(&mut self, parent: Element) -> Result<Option<Vec<u8>>> {
        let mut offset = parent.start;
        while let Some(element) = self.next(&mut offset, parent.end)? {
            if element.id == 0x61a7 {
                if let Some(bytes) = self.attached_file(element)? {
                    return Ok(Some(bytes));
                }
            }
        }
        Ok(None)
    }

    fn segment(&mut self, parent: Element) -> Result<Option<Vec<u8>>> {
        let mut offset = parent.start;
        while let Some(element) = self.next(&mut offset, parent.end)? {
            if element.id == 0x1941a469 {
                if let Some(bytes) = self.attachments(element)? {
                    return Ok(Some(bytes));
                }
            }
        }
        Ok(None)
    }
}

pub(super) fn read(file: &mut File, length: u64) -> Result<Option<Vec<u8>>> {
    let mut scan = Scan { file, records: 0 };
    let mut offset = 0;
    while let Some(element) = scan.next(&mut offset, length)? {
        if element.id == 0x18538067 {
            return scan.segment(element);
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "matroska_cover_tests.rs"]
mod tests;
