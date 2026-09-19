# Live Production Guide

Streaming a stereo program bus from a DAW on one machine to a live switcher on another. The worked example is Logic Pro on macOS feeding vMix on Windows for a YouTube Live stream, but the shape applies to any DAW and switcher pair.

```
Mixer ──USB──▶ Mac / Logic Pro ──▶ BlackHole 2ch ──▶ TCP Streamer (Source, UDP)
                                                            │
                                                          LAN
                                                            ▼
                     vMix ◀── VB-Cable ◀── TCP Streamer (Sink, UDP) / Windows
```

## Before you start

**Use wired gigabit Ethernet, both machines on the same switch.** Do not use Wi-Fi. Lost UDP packets are concealed, not retransmitted, and concealment is audible on exposed program material.

**This is a stereo path.** A multichannel capture device contributes only its front left/right pair, so Logic must sum everything to one stereo bus before it leaves the machine. Running several instances for separate stems does not work: each instance keeps its own buffer and its own drift controller, so the stems drift out of alignment with each other.

## macOS source

1. Install a virtual audio device: `brew install blackhole-2ch`.
2. In Logic Pro, route the stereo output bus to **BlackHole 2ch**.
3. To keep monitoring locally, open **Audio MIDI Setup**, create a **Multi-Output Device** containing both BlackHole 2ch and your normal output, and select that as Logic's output instead.
4. In TCP Streamer:
   - Role: **Source**
   - Transport: **Native UDP**
   - Input device: **BlackHole 2ch**
   - Output sample rate: **48000**
   - Capture buffer: **256** or **512**
   - Latency profile: **Broadcast**

## Windows sink

1. In TCP Streamer:
   - Role: **Sink**
   - Latency profile: **Broadcast**, with the same fixed latency as the source
   - Source: pick the Mac from the mDNS list, or enter `host:port` manually
2. Choose how the audio reaches vMix — two options, both documented below.

### Option A: VB-Audio Cable (recommended)

Install [VB-Audio Cable](https://vb-audio.com/Cable/). Set TCP Streamer's **output device** to `CABLE Input`, then in vMix add an **Audio Input** and select `CABLE Output`.

Only TCP Streamer's audio reaches vMix. Nothing else on the machine can leak into the broadcast.

### Option B: Stereo Mix (no extra software)

Some sound cards expose a **Stereo Mix** recording device. Where it exists, TCP Streamer can play to the normal output and vMix can take Stereo Mix as an audio input, with nothing else installed.

Two caveats, both real:

- It captures **all** system audio, including notification sounds and anything else playing. Those go out on the broadcast.
- Many modern audio devices no longer expose it at all. If it is not in the Windows recording device list, use Option A.

## Audio/video sync

The Broadcast profile exists for this step. Under any other profile the adaptive controller raises the latency target when it sees a glitch and lowers it after a stable period, so a compensation measured at the top of the show stops being correct partway through.

1. Start the source and the sink. Let the link settle for a minute.
2. Clap in front of the camera, in frame, while recording vMix's program output.
3. Open the recording and measure the offset between the clap in the picture and the clap in the audio.
4. Set vMix's video input delay to that measurement.

Measure once per venue setup. The offset holds as long as both ends stay on the Broadcast profile with the same fixed latency.

### Choosing a fixed latency

The default is 250 ms, which suits a quiet wired LAN. Raise it if the dashboard reports underruns; there is no adaptation to rescue a target the network cannot meet. Lower it only on a link you have already watched behave.

## Clock drift

The Mac's capture clock and the Windows playback clock run independently — nothing synchronises them, and they will diverge slowly.

TCP Streamer keeps the buffer at its target by occasionally dropping a small chunk of audio or inserting a small amount of silence. This bounds the latency, which is what keeps your A/V measurement valid, but each correction is a brief micro-glitch. Corrections are rare: the controller only acts when its smoothed buffer level strays past a quarter of the target, and it waits out a cooldown between corrections.

TCP Streamer does not use an asynchronous sample-rate converter, which is how broadcast-grade equipment avoids these corrections entirely. If your programme material cannot tolerate any correction at all, use hardware with a shared clock.

## Limits

- Stereo only. No multichannel, no stems.
- No forward error correction. A lossy link produces concealment, not recovery.
- No redundant path or automatic failover.
- The sink requires a Native UDP source. TCP sources are for external receivers such as Snapcast.
