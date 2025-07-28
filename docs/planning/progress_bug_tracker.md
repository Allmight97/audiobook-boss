# Bugs and Features
[ ] BUG: 'clear cover art' button IS NOT visible in the UI when I click on a file in the file list. It is  only visible when I load cover Art with load cover Art button.
    - ACTION: invoke 'clear cover art' when cover art is loaded FROM ANY SOURCE.
[ ] FEATURE: Add ability to clear loaded files from file list.
    - ACTION: Add a 'clear' button to the file list using as minimal code as possible.
[ ] FIX: Why does terminal output say "Starting FFmpeg merge" - Total duration: 35740.08s, Bitrate: 56k
Converting: 23.3% (8310.0s / 35740.1s) - when I'm only loading 1 file?
    - ACTION: INvestigate - does this impact the app at all front or back end? And does the message imply FFMPEG is doing something it shouldn't be or is "starting FFmpeg merge" simply a placeholder message?
[ ] FEATURE: Add ability to process multiple files loaded into the file list as separate jobs (single audiobook per file), outputing to different directories custom to each file.  E.G. I have 4 books that need shrunk all different books by the same author frome the same series. All books should save to the same parent directory that matches the author name, but each book should save to a different directory if I choose the option to save to a different directory.