# Output artifact core

## Outside facts

- A file or folder name holds at most 255 units: UTF-16 units on APFS, HFS+,
  exFAT, NTFS, and SMB; bytes on Linux disks (ext4, btrfs). `name_length`
  counts the unit of the build's platform.
