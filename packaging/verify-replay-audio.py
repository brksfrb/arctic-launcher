"""Verify actual decoded clip audio and A/V duration, not just an MP4 audio-track flag."""
import array
import json
from pathlib import Path
import subprocess
import sys

clips = list(Path(sys.argv[1]).glob('*Clip*.mp4'))
assert clips, 'No instant replay MP4 was exported'
for clip in clips:
    info = json.loads(subprocess.check_output(['ffprobe', '-v', 'error', '-show_streams', '-of', 'json', str(clip)]))
    video = next(s for s in info['streams'] if s['codec_type'] == 'video')
    audio = next(s for s in info['streams'] if s['codec_type'] == 'audio')
    assert abs(float(video['duration']) - float(audio['duration'])) < 0.08, 'Audio/video drift'
    pcm = array.array('h', subprocess.check_output(['ffmpeg', '-v', 'error', '-threads', '1', '-i', str(clip), '-vn', '-f', 's16le', '-ac', '2', '-ar', '48000', '-']))
    peak = max(abs(n) for n in pcm)
    assert peak > 100, f'Audio is silent: peak {peak}'
    print(f'{clip.name}: audible AAC, peak {peak}, A/V duration matched')
