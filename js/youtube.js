import { fetch, CustomEvent } from "./rust-apis.js";
import {
  Innertube,
  Misc,
  Log,
  Platform,
  YT,
  YTNodes,
} from "youtubei.js/web.bundle";

Log.setLevel(Log.Level.NONE);
Platform.load({
  runtime: "unknown",
  server: true,
  fetch,
  Request,
  Headers,
  CustomEvent,
});

export function parseText(json) {
  const text = new Misc.Text(JSON.parse(json));
  return text.toString();
}

export async function sessionName() {
  const yt = await Innertube.create({
    retrieve_player: false,
    generate_session_locally: true,
    retrieve_innertube_config: false,
    timezone: "UTC",
    lang: "en",
    location: "US",
  });
  return yt.session.context.client.clientName;
}

export async function playlistPages(json) {
  const { id } = JSON.parse(json);
  const yt = await Innertube.create({
    retrieve_player: false,
    generate_session_locally: true,
    retrieve_innertube_config: false,
    timezone: "UTC",
    lang: "en",
    location: "US",
  });
  const response = await yt.actions.execute("/browse", {
    browseId: `VL${id}`,
    params: "wgYCCAA=",
  });
  let page = new YT.Playlist(yt.actions, response);
  const videos = [];
  const pages = new Set();
  while (true) {
    if (
      page.page.alerts?.some(
        (alert) =>
          alert.is(YTNodes.Alert, YTNodes.AlertWithButton) &&
          alert.alert_type !== "INFO",
      )
    )
      throw new Error("Incomplete playlist: YouTube returned an alert");
    const fingerprint = page.items
      .map((item) => item.id || item.content_id)
      .join(",");
    if (pages.has(fingerprint) || pages.size >= 10000)
      throw new Error("Repeated playlist continuation");
    pages.add(fingerprint);
    for (const item of page.items) {
      if (item.is(YTNodes.PlaylistVideo))
        videos.push({
          id: item.id,
          title: item.title.toString(),
          duration: item.duration.seconds || 0,
          published: item.video_info.toString(),
          available:
            item.is_playable &&
            !item.is_live &&
            !item.is_upcoming &&
            !item.upcoming,
        });
      else if (
        item.is(YTNodes.LockupView) &&
        ["VIDEO", "SHORT"].includes(item.content_type)
      )
        videos.push({
          id: item.content_id,
          title: item.metadata?.title.toString() || item.content_id,
          duration: 0,
          published:
            item.metadata?.metadata?.metadata_rows
              .at(-1)
              ?.metadata_parts?.at(-1)
              ?.text?.toString() || "",
          available:
            !!item.metadata?.title.toString() && !hasLiveOrUpcomingBadge(item),
        });
      else throw new Error("Unsupported playlist renderer");
    }
    if (!page.has_continuation) break;
    page = await page.getContinuation();
  }
  return JSON.stringify(videos);
}

function hasLiveOrUpcomingBadge(item) {
  if (!item.content_image?.is(YTNodes.ThumbnailView)) return false;
  return item.content_image.overlays.some(
    (overlay) =>
      overlay.is(
        YTNodes.ThumbnailOverlayBadgeView,
        YTNodes.ThumbnailBottomOverlayView,
      ) &&
      overlay.badges.some(
        (badge) =>
          badge.badge_style === "THUMBNAIL_OVERLAY_BADGE_STYLE_LIVE" ||
          badge.text?.trim().toLowerCase() === "upcoming",
      ),
  );
}

globalThis.youtubeiNative = { parseText, sessionName, playlistPages };
