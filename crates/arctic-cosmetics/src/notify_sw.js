// Arctic Notifications: shows a pushed note and opens the dashboard where it points when it is tapped.
self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));

self.addEventListener("push", (event) => {
  let note = { title: "Arctic", body: "", url: "/admin", tag: "arctic" };
  try {
    note = Object.assign(note, event.data ? event.data.json() : {});
  } catch (e) {
    // A note that isn't JSON still shows.
    note.body = event.data ? event.data.text() : "";
  }
  event.waitUntil(
    self.registration.showNotification(note.title, {
      body: note.body,
      tag: note.tag,
      icon: "/notify/icon-192.png",
      badge: "/notify/icon-192.png",
      data: { url: note.url },
    })
  );
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const url = (event.notification.data && event.notification.data.url) || "/admin";
  event.waitUntil(
    self.clients.matchAll({ type: "window", includeUncontrolled: true }).then((windows) => {
      for (const w of windows) {
        if ("focus" in w && new URL(w.url).pathname === new URL(url, self.location.origin).pathname) {
          return w.focus();
        }
      }
      return self.clients.openWindow(url);
    })
  );
});
