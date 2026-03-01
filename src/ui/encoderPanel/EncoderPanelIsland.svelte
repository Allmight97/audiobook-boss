<div
  id="encoder-settings-panel"
  class="section-divider"
  data-testid="encoder-settings-panel"
>
  <div class="section-header">
    <h3>Audio Encoder Settings</h3>
    <span class="inline-info">
      (<span id="estimated-size" data-testid="estimated-size">~ --- MB</span>)
    </span>
  </div>

  <div class="grid grid-cols-4 gap-x-3 gap-y-2 mb-2">
    <div>
      <label for="adv-encoder">Encoder</label>
      <select id="adv-encoder" data-testid="encoder-select">
        <option value="auto" selected>Auto</option>
        <option value="fdk_he_aac">FDK AAC</option>
        <option value="aac_at">Apple AAC</option>
        <option value="native_aac">Native AAC (FFmpeg)</option>
      </select>
      <p
        id="encoder-availability-hint"
        class="text-xs muted-text mt-0.5"
        data-testid="encoder-availability-hint"
      >
        Checking…
      </p>
    </div>
    <div>
      <span class="label">Profile</span>
      <div
        id="encoder-profile-display"
        class="profile-display"
        data-testid="profile-display"
      >
        HE-AAC v1
      </div>
      <p class="text-xs muted-text mt-0.5 italic">Read-only</p>
    </div>
    <div>
      <label for="adv-bitrate-mode">Bitrate Mode</label>
      <select id="adv-bitrate-mode" data-testid="bitrate-mode-select">
        <option value="vbr" selected>VBR</option>
        <option value="cvbr">CVBR</option>
        <option value="cbr">CBR</option>
      </select>
    </div>
    <div>
      <label for="output-quality" id="quality-bitrate-label">Quality</label>
      <select id="output-quality" data-testid="quality-select">
        <option value="1">1 (Smallest)</option>
        <option value="2">2</option>
        <option value="3" selected>3 (Recommended)</option>
        <option value="4">4</option>
        <option value="5">5 (Largest)</option>
      </select>
      <select id="output-bitrate" class="hidden" data-testid="bitrate-select">
        <option value="48">48 kbps</option>
        <option value="56">56 kbps</option>
        <option value="64" selected>64 kbps</option>
        <option value="72">72 kbps</option>
        <option value="80">80 kbps</option>
        <option value="88">88 kbps</option>
        <option value="96">96 kbps</option>
        <option value="104">104 kbps</option>
        <option value="112">112 kbps</option>
        <option value="120">120 kbps</option>
        <option value="128">128 kbps</option>
      </select>
      <p
        id="estimated-bitrate"
        class="text-xs muted-text mt-0.5"
        data-testid="estimated-bitrate"
      >
        Est: ~60 kbps
      </p>
    </div>
  </div>

  <div class="grid grid-cols-4 gap-x-3 gap-y-2 mb-2">
    <div>
      <label for="output-samplerate">Sample Rate</label>
      <select id="output-samplerate" data-testid="samplerate-select">
        <option value="auto" selected>Auto</option>
        <option value="22050">22050 Hz</option>
        <option value="32000">32000 Hz</option>
        <option value="44100">44100 Hz</option>
        <option value="48000">48000 Hz</option>
      </select>
      <p
        id="output-samplerate-effective"
        class="text-xs muted-text mt-0.5"
        data-testid="auto-samplerate-hint"
      >
        Auto resolves from source audio.
      </p>
    </div>
    <div>
      <label for="output-channels">Channels</label>
      <select id="output-channels" data-testid="channels-select">
        <option value="auto" selected>Auto</option>
        <option value="mono">Mono</option>
        <option value="stereo">Stereo</option>
      </select>
      <p
        id="output-channels-effective"
        class="text-xs muted-text mt-0.5"
        data-testid="auto-channels-hint"
      >
        Auto resolves from source audio.
      </p>
    </div>
    <div class="col-span-2 flex items-end"></div>
  </div>

  <div class="flex gap-4 items-center h-full">
    <div
      id="fdk-options"
      class="encoder-option-group"
      data-testid="fdk-options"
    >
      <label
        class="checkbox-label mb-0"
        data-testid="afterburner-toggle"
        title="Enable Afterburner"
      >
        <input
          type="checkbox"
          id="adv-fdk-afterburner"
          checked
          data-testid="afterburner-checkbox"
        />
        <span class="option-label">Afterburner</span>
      </label>
    </div>

    <div
      id="native-options"
      class="encoder-option-group hidden"
      data-testid="native-options"
    >
      <label
        class="checkbox-label mb-0"
        data-testid="twoloop-toggle"
        title="Enable Twoloop (High Quality)"
      >
        <input
          type="checkbox"
          id="adv-native-twoloop"
          checked
          data-testid="twoloop-checkbox"
        />
        <span class="option-label">Twoloop</span>
      </label>
    </div>
  </div>
</div>
