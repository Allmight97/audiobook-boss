# Bugs and Features
[X] BUG: 'clear cover art' button IS NOT visible in the UI when I click on a file in the file list. It is  only visible when I load cover Art with load cover Art button.
    - ACTION: invoke 'clear cover art' when cover art is loaded FROM ANY SOURCE.
[X] FEATURE: Add ability to clear loaded files from file list.
    - ACTION: Add a 'clear' button to the file list using as minimal code as possible.
[X] FIX: Why does terminal output say "Starting FFmpeg merge" - Total duration: 35740.08s, Bitrate: 56k
Converting: 23.3% (8310.0s / 35740.1s) - when I'm only loading 1 file?
    - ACTION: INvestigate - does this impact the app at all front or back end? And does the message imply FFMPEG is doing something it shouldn't be or is "starting FFmpeg merge" simply a placeholder message?
[ ] FEATURE: Add ability to process multiple files loaded into the file list as separate jobs (single audiobook per file), outputing to different directories custom to each file.  E.G. I have 4 books that need shrunk all different books by the same author frome the same series. All books should save to the same parent directory that matches the author name, but each book should save to a different directory if I choose the option to save to a different directory.
[ ] FEATURE: Add ability to load cover art from URL.
    - ACTION: Work with agent to design and implement this feature. Perhaps a simply load URL button?
[ ] BUG: Loaded cover art is replaced with whatever was imported from the input file the moment I click on another file in the file list.
    - CONTEXT:cover art correctly loads when pressing 'load cover art' button and correctly clears when pressing 'clear cover art' button. In the case of editing several audio files from the same book.
    - DESIRED BEHAVIOR: Cover art should be preserved when I click on another file in the file list. Clearing should only occur when I press 'clear cover art' button; when I load a new file or clear the file list; when I override the cover art with a new file.

[X] BUG: Noticing a warning about lofty since completing p1.1.3_progress_split
    [2025-08-11T23:41:09Z INFO  audiobook_boss_lib] Starting Audiobook Boss application
        [2025-08-11T23:46:21Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration

[X] BUG: Output M4B file does NOT have Cover Art at all - regardless of whether I load cover art manually or from the input file when setting up output file for processing. [FIXED]
[X] BUG: "cancel processing" button doesn't cancel current process. Nothing shows in DOM console nor terminal output as registered click of the button. When I press it there appears to be split second change in the "current step: ..." text display as the job processes, but no change in behavior. [FIXED]

[ ] FEATURE: Give users ability to choose FDK-AAC if they have it installed on their local hardware (we cannot legally ship FDK-AAC with the app!)
    - Action: Work up a plan to inject this option to the output panel settings and give the user the ability to set the path of FDK-AAC, or possibly have the app be able to detect it automatically if option is ticked. Manual option is best for now due to presumed implementation complexity. Plan should also include steps to default ffmpeg-next encoder to use native AAC-LC.
        - SEE docs/reports/AAC_advice.md for the advice I was given for this feature and why.

[ ] TODO: 100% remove shellFFMpeg from codebase such that codebase is only using ffmpeg-next.
    - Action: No feature gates nor safeffmpeg - default encode will be ffmpeg-next using standard AAC-LC
        - Consider UI options to allow use to apply the 'twoloop' flag to enhance audio quality of native AAC encoder: e.g. 'ffmpeg -i input.mp3 -c:a aac -aac_coder twoloop -b:a 64k output.m4b'
[ ] TODO: How to add these FFmpeg sources to repo so Ai agents can easily reference them when coding and auditing?
        - docs.rs/ffmpeg-next
        - ffmpeg.org/ffmpeg.html
    - Detailed chat and context: https://www.perplexity.ai/search/i-just-heard-about-ffmpeg-8-0-caDGTIhOSDeSNn2FJKoU0Q