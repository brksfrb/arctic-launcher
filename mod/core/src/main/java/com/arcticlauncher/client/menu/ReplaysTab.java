package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.LocalClock;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.replay.ReplayMenu;
import com.arcticlauncher.client.replay.Replays;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Label;
import com.arcticlauncher.client.ui.TextField;
import com.arcticlauncher.client.ui.Widget;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import java.util.Date;
import java.util.List;
import java.util.Locale;

import com.arcticlauncher.client.Lang;
/**
 * Instant clips, manual recording controls, queued videos and saved replays.
 */
final class ReplaysTab implements MenuTab {
	private static final int ROW_H = 38;
	private static final int THUMB_W = 48;
	private static final int BTN_H = 16;
	private static final int WATCH_W = 48;
	private static final int CLIP_W = 44;
	private static final int DELETE_W = 44;
	private static final int GAP = 4;
	private static final int MIN_CLIP = 10;
	private static final int MAX_CLIP = 120;
	private static final int CLIP_STEP = 5;
	private final TextField search = new TextField(Lang.t("replay.search_saved"), 80, TextField.ANY);
	private Replays.Info renaming;
	private String renameError;
	private boolean bindings;

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
		return Replays.list().hashCode() + ":" + Replays.recorder().state() + ":" + Replays.openProblem() + ":" + c.replayEnabled + ":" + Replays.queuedVideos()
				+ ":" + (Replays.liveVideo() == null ? "" : Replays.liveVideo().status())
				+ ":" + Replays.ffmpeg().state() + ":" + c.replayKey + ":" + c.replayRecordKey + ":" + Replays.pendingSaves();
	}

	@Override
	public void build(final Host host, int x, int top, int w, int bottom) {
		final ClientConfig c = ArcticClient.config();
		Form f = new Form(host, x, top, w, bottom);
		f.toggle(Lang.t("replay.replay"), Lang.t("replay.replay_saving_description"), () -> c.replayEnabled, on -> { c.replayEnabled = on; Replays.recorder().enabled(on); host.rebuild(); });
		if (!c.replayEnabled) {
			f.note(Lang.t("replay.replay_is_off_saved_replays_below_still_play"));
		}
		f.section(Lang.t("replay.recording"));
		f.key(Lang.t("replay.save_instant_clip"), Lang.t("replay.saves_recent_gameplay_video_3d_your"), Form.key(() -> c.replayKey, k -> c.replayKey = k));
		f.key(Lang.t("replay.start_stop_recording"), Lang.t("replay.begins_now_saves_when_stopped_or_when_you"), Form.key(() -> c.replayRecordKey, k -> c.replayRecordKey = k));
		if (c.replayKey.equals(c.replayRecordKey)) f.note(Lang.t("replay.choose_different_keys_for_the_two_actions"));
		f.stepper(Lang.t("replay.clip_length"), () -> c.clipSeconds, v -> c.clipSeconds = (int) v, MIN_CLIP, MAX_CLIP, CLIP_STEP, "%.0fs");
		f.note(Replays.recordingStatus());
		f.key(Lang.t("replay.mark_recording"), Lang.t("replay.add_marker_at_playhead"), Form.key(() -> c.replayMarkerKey, key -> c.replayMarkerKey = key));
		f.put(new Button(Lang.t("replay.timeline_controls"), () -> { bindings = !bindings; host.rebuild(); }),
				f.x, f.row(23), 150, 18);
		if (bindings) {
			for (Replays.ViewerAction action : Replays.ViewerAction.values()) {
				f.key(Lang.t(action.label), "", Form.key(action::binding, action::bind));
			}
		}
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
		if (Replays.openProblem() != null) f.note(Replays.openProblem());
		if (renameError != null) f.note(renameError);
		search.onChange(host::rebuild);
		f.put(search, f.x, f.row(23), f.w, 18);
		if (replays.isEmpty()) f.note(Lang.t("replay.press_the_replay_key_while_playing_to_keep"));
		boolean inWorld = ArcticClient.platform().inWorld();
		String query = search.text().trim().toLowerCase(Locale.ROOT);
		for (final Replays.Info info : replays) {
			if (!query.isEmpty() && !info.name.toLowerCase(Locale.ROOT).contains(query)
					&& !details(info).toLowerCase(Locale.ROOT).contains(query)) continue;
			int row = f.row(ROW_H);
			int tx = f.x + THUMB_W + GAP * 2;
			int bx = f.x + f.w - DELETE_W - CLIP_W - WATCH_W - GAP * 2;
			f.put(new Widget() {
				@Override protected void draw(Gfx g, Style style, int mx, int my, float dt) {
					g.fill(x, y, x + w, y + h, style.border);
					if (info.thumbnail != null && ArcticClient.looks().isReady(info.thumbnail)) {
						g.texture("look:" + info.thumbnail, x, y, w, h, 0, 0,
								info.thumbnailWidth, info.thumbnailHeight, info.thumbnailWidth, info.thumbnailHeight);
					}
				}
			}, f.x + GAP, row + 2, THUMB_W, 27);
			f.put(new Label(info.name, Label.Kind.TEXT), tx, row + 2, Math.max(16, bx - tx - GAP), 9);
			f.put(new Label(details(info), Label.Kind.MUTED), tx, row + 14, Math.max(16, bx - tx - GAP), 9);
			int by = row + 1;
			boolean compatible = Replays.compatible(info);
			Button watch = new Button(Lang.t("replay.watch"), () -> Replays.watch(info.file)).primary();
			if (inWorld) watch.confirm(Lang.t("replay.leave"));
			if (compatible) f.put(watch, bx, by, WATCH_W, BTN_H);
			else f.put(new Button(Lang.t("replay.watch_in_launcher"), () -> Replays.openInLauncher(info))
					.enabled(Replays.canOpenInLauncher()), bx, by, WATCH_W + CLIP_W + GAP, BTN_H);
			final int moment = info.extras.moments.isEmpty() ? -1 : info.extras.moments.get(info.extras.moments.size() - 1);
			Button clip = new Button(Lang.t("replay.render"), () -> Replays.watch(info.file, v -> ReplayMenu.exportClip(v, moment)));
			if (inWorld) clip.confirm(Lang.t("replay.leave"));
			if (compatible) f.put(clip.enabled(moment >= 0), bx + WATCH_W + GAP, by, CLIP_W, BTN_H);
			f.put(new Button(Lang.t("replay.delete"), () -> {
				if (!Replays.delete(info)) renameError = Lang.t("replay.replay_delete_failed");
				host.rebuild();
			}).confirm(Lang.t("replay.sure")), bx + WATCH_W + CLIP_W + GAP * 2, by, DELETE_W, BTN_H);
			f.put(new Button(Lang.t("replay.rename_replay"), () -> {
				renaming = info;
				renameError = null;
				host.rebuild();
			}), bx, by + BTN_H + 2, WATCH_W + CLIP_W + GAP, BTN_H);
			if (renaming == info) {
				int fieldRow = f.row(24);
				TextField name = new TextField(Lang.t("replay.rename_replay"), 100, TextField.ANY).text(info.name);
				Runnable rename = () -> {
					renameError = Replays.rename(info, name.text());
					if (renameError == null) renaming = null;
					host.rebuild();
				};
				f.put(name.onEnter(rename), tx, fieldRow, Math.max(16, f.x + f.w - tx - WATCH_W - GAP), 18);
				f.put(new Button(Lang.t("replay.rename_replay"), rename).primary(), f.x + f.w - WATCH_W, fieldRow, WATCH_W, 18);
			}
		}
		f.done();
	}

	private static String details(Replays.Info info) {
		String when = LocalClock.formatter("MMM d, HH:mm", Locale.ENGLISH).format(new Date(info.meta.date));
		int clips = info.extras.moments.size();
		String size = String.format(Locale.ROOT, "%.0f MB", info.size / (1024.0 * 1024.0));
		return when + " · " + Replays.clock(info.meta.duration) + " · " + (clips == 1 ? "1 clip" : clips + " clips")
				+ " · " + info.meta.mcversion + " · " + size;
	}
}
