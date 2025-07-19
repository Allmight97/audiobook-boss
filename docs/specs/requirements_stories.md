# Audiobook Boss MVP.

## **Executive Summary**

AudioBook Boss is a desktop application for merging and processing audiobook files with metadata control. The initial version will only run on the Dev's (JStar) personal Macbook Pro (m1 Max, 32Gb mem). Future versions will support cross platform and possible additional complex features like Auidble library sync and "watched folder" support - neither of which will be included in the MVP.

## **Core Problem**

Users need to merge multiple audio files (MP3/M4A/M4B/AAC) into a single M4B audiobook with proper metadata, compression settings, and file organization \- currently requiring multiple tools and manual processes.

## **User Stories & BDD Scenarios**

### **Story 1: File Import and Management**

**As a user, I want to import multiple audio files so I can prepare them for merging**

Scenario: Drag and drop audio files

- Given the application is open  
- When I drag MP3/M4A/M4B/AAC files onto the drop zone  
- Then the files appear in the file list in selection order  
- And I see the combined total size  
- And non-audio files are ignored

Scenario: Reorder files for correct sequence

- Given I have multiple files in the list  
- When I drag a file to a new position  
- Then the file order updates  
- And the order persists for processing

### **Story 2: File Property Inspection**

**As a user, I want to see audio properties so I can make informed processing decisions**

Scenario: View single file properties

- Given files are loaded  
- When I select a single file  
- Then I see its bitrate, sample rate, channels, and size  
- And the properties display updates within 1 second

### **Story 3: Metadata Editing**

**As a user, I want to edit book metadata so my audiobook is properly cataloged**

Scenario: Load and edit metadata

- Given I have files loaded  
- When the first file has metadata  
- Then it auto-populates the metadata fields  
- And I can edit: Title, Author, Album, Narrator, Year, Genre, Series info, Description

Scenario: Add cover art

- Given I'm editing metadata  
- When I click "Load Cover Art" or drag an image  
- Then the cover art displays  
- And it will be embedded in the output

### **Story 4: Output Configuration**

**As a user, I want to configure output settings so I can control file size and quality**

Scenario: Configure audio settings

- Given I'm preparing to process  
- When I select output settings  
- Then I can choose:  
  - Bitrate (32-128 kbps, default 64\)  
  - Channels (Mono/Stereo, default Mono)  
  - Sample rate (Pass-through by default)  
- And I see estimated output size update

Scenario: Configure output location

- Given I'm configuring output  
- When I set the output directory  
- Then I can choose between:  
  - Default pattern: \[Base\]/\[Author\]/\[Series\]/\[Year-Title\]/  
  - Custom base directory only  
- And I see the calculated full path

### **Story 5: Preview Generation**

**As a user, I want to preview my settings so I can verify quality before full processing**

Scenario: Generate preview

- Given I have configured all settings  
- When I click "Preview (30s)"  
- Then a 30-second sample is generated with my settings  
- And it opens in my default audio player  
- And uses the configured metadata and cover art

### **Story 6: Full Processing**

**As a user, I want to process my audiobook so I can have a single properly-formatted M4B file**

Scenario: Process audiobook successfully

- Given all settings are configured  
- When I click "Process Audiobook"  
- Then I see real-time progress (percentage and time)  
- And the button changes to "Cancel Processing"  
- And the final M4B has all my metadata and settings

Scenario: Cancel processing

- Given processing is in progress  
- When I click "Cancel Processing"  
- Then FFmpeg stops cleanly  
- And partial files are removed  
- And the UI returns to ready state

### **Story 7: Input Validation**

**As a user, I want the app to validate my inputs so I avoid processing errors**

Scenario: Validate before processing

- Given I click "Process Audiobook"  
- When validation runs  
- Then it checks:  
  - At least one input file exists  
  - Output directory is writable  
  - Required metadata fields are filled  
  - No filename conflicts  
- And shows clear error messages for any issues

## **Technical Architecture**
PENDING - work in progress - final tech stack undecided. Leaning towards [docs/specs/MVP Design.md](../specs/MVP%20Design.md)