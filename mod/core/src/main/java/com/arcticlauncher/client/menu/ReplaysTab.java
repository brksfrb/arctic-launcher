package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.LocalClock;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.replay.ReplayMenu;
import com.arcticlauncher.client.replay.Replays;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Label;
import java.util.Date;
import java.util.List;
import java.util.Locale;

import com.arcticlauncher.client.Lang;
/**
 * Instant clips, manual recording controls, queued videos and saved replays.
 */
final class ReplaysTab implements MenuTab {
	private static final int ROW_H = 30;
	private static final int BTN_H = 16;
	private static final int WATCH_W = 48;
	private static final int CLIP_W = 44;
	private static final int DELETE_W = 44;
	private static final int GAP = 4;
	private static final int MIN_CLIP = 10;
	private static final int MAX_CLIP = 120;
	private static final int CLIP_STEP = 5;

	@Override
	public String title() {
		return Lang.t("replay.replays");
	}

	@Override
	public String hint() {
		return Lang.t("replay.save_a_moment_or_record_from_start_to_stop");
	}

	@Override
	public String state() {
		ClientConfig c = ArcticClient.config();
		return Replays.list().size() + ":" + Replays.recording() + ":" + Replays.queuedVideos()
				+ ":" + (Replays.liveVideo() == null ? "" : Replays.liveVideo().status())
				+ ":" + Replays.ffmpeg().state() + ":" + c.replayKey + ":" + c.replayRecordKey + ":" + Replays.pendingSaves();
	}

	@Override
	public void build(final Host host, int x, int top, int w, int bottom) {
		final ClientConfig c = ArcticClient.config();
		Form f = new Form(host, x, top, w, bottom);
		f.toggle(Lang.t("replay.replay"), Lang.t("replay.instant_clips_recordings_and_gameplay_video"), () -> c.replayEnabled, on -> { c.replayEnabled = on; host.rebuild(); });
		if (!c.replayEnabled) {
			f.note(Lang.t("replay.replay_is_off_saved_replays_below_still_play"));
		}
		f.section(Lang.t("replay.recording"));
		f.key(Lang.t("replay.save_instant_clip"), Lang.t("replay.saves_recent_gameplay_video_3d_your"), Form.key(() -> c.replayKey, k -> c.replayKey = k));
		f.key(Lang.t("replay.start_stop_recording"), Lang.t("replay.begins_now_saves_when_stopped_or_when_you"), Form.key(() -> c.replayRecordKey, k -> c.replayRecordKey = k));
		if (c.replayKey.equals(c.replayRecordKey)) f.note(Lang.t("replay.choose_different_keys_for_the_two_actions"));
		f.stepper(Lang.t("replay.clip_length"), () -> c.clipSeconds, v -> c.clipSeconds = (int) v, MIN_CLIP, MAX_CLIP, CLIP_STEP, "%.0fs");
		f.note(Replays.recordingStatus());
		if (ArcticClient.platform().inWorld()) {
			int row = f.row(24);
			f.put(new Button(Replays.recording() ? Lang.t("replay.stop_and_save_recording") : Lang.t("replay.start_recording"), () -> { Replays.toggleRecording(); host.rebuild(); }), f.x, row, 160, 18);
		}
		f.section(Lang.t("replay.gameplay_videos") + (Replays.queuedVideos() > 0 ? Lang.t("replay.saving_count", Replays.queuedVideos()) : ""));
		f.toggle(Lang.t("replay.capture_gameplay_video"), Lang.t("replay.records_the_game_window_with_audio_while_you"), () -> c.replayVideoEnabled, on -> c.replayVideoEnabled = on);
		f.choice(Lang.t("replay.video_resolution"), new String[]{"720p", "1080p"}, () -> c.replayVideoHeight >= 1080 ? 1 : 0, i -> c.replayVideoHeight = i == 1 ? 1080 : 720);
		f.choice(Lang.t("replay.video_frame_rate"), new String[]{"30 FPS", "60 FPS"}, () -> c.replayVideoFps >= 60 ? 1 : 0, i -> c.replayVideoFps = i == 1 ? 60 : 30);
		f.note(Lang.t("replay.changes_apply_on_the_next_world_join_higher"));
		if (Replays.liveVideo() != null) f.note(Replays.liveVideo().status());
		f.note(Lang.t("replay.f9_saves_the_recent_video_start_stop_saves"));
		int videoRow = f.row(24);
		f.put(new Button(Lang.t("replay.open_videos_folder"), () -> com.arcticlauncher.client.feature.FolderOpener.open(Replays.videoFolder())),
				f.x, videoRow, 140, 18);
		if (Replays.legacyQueuedVideos() > 0) f.note(Lang.t("replay.older_clips_have_3d_data_only_render_them"));
		if (Replays.liveVideo() != null && Replays.liveVideo().encoder().get() == null) {
			int row = f.row(24);
			f.put(new Button(Lang.t("replay.get_video_encoder"), () -> Replays.liveVideo().encoder().download()), f.x, row, 140, 18);
		}

		List<Replays.Info> replays = Replays.list();
		f.section(Lang.t("replay.saved") + replays.size() + ")");
		if (replays.isEmpty()) {
			f.note(Lang.t("replay.press_the_replay_key_while_playing_to_keep"));
		}
		boolean inWorld = ArcticClient.platform().inWorld();
		for (final Replays.Info info : replays) {
			int row = f.row(ROW_H);
			int textW = f.w - WATCH_W - CLIP_W - DELETE_W - GAP * 4;
			f.put(new Label(info.name, Label.Kind.TEXT), f.x + 4, row + 4, textW, 9);
			f.put(new Label(details(info), Label.Kind.MUTED), f.x + 4, row + 15, textW, 9);
			int bx = f.x + f.w - DELETE_W - CLIP_W - WATCH_W - GAP * 2;
			int by = row + (ROW_H - BTN_H) / 2;
			Button watch = new Button(Lang.t("replay.watch"), () -> Replays.watch(info.file)).primary();
			if (inWorld) {
				watch.confirm(Lang.t("replay.leave"));
			}
			f.put(watch.enabled(Replays.canWatch()), bx, by, WATCH_W, BTN_H);
			bx += WATCH_W + GAP;
			final int moment = info.extras.moments.isEmpty() ? -1 : info.extras.moments.get(info.extras.moments.size() - 1);
			Button clip = new Button(Lang.t("replay.render"), () -> Replays.watch(info.file, v -> ReplayMenu.exportClip(v, moment)));
			if (inWorld) clip.confirm(Lang.t("replay.leave"));
			f.put(clip.enabled(Replays.canWatch() && moment >= 0), bx, by, CLIP_W, BTN_H);
			bx += CLIP_W + GAP;
			f.put(new Button(Lang.t("replay.delete"), () -> {
				Replays.delete(info);
				host.rebuild();
			}).confirm(Lang.t("replay.sure")), bx, by, DELETE_W, BTN_H);
		}
		f.done();
	}

	private static String details(Replays.Info info) {
		String when = LocalClock.formatter("MMM d, HH:mm", Locale.ENGLISH).format(new Date(info.meta.date));
		int clips = info.extras.moments.size();
		String size = String.format(Locale.ROOT, "%.0f MB", info.file.length() / (1024.0 * 1024.0));
		return when + " · " + Replays.clock(info.meta.duration) + " · " + (clips == 1 ? "1 clip" : clips + " clips")
				+ " · " + info.meta.mcversion + " · " + size;
	}
}
