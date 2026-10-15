# Website SEO brief (for the "Arctic Launcher website" session)

Written 2026-10-16. Source: a read of the live site (arcticlauncher.com, its sitemap, robots.txt),
the GitHub repo and README. Goal: be found by people who want a Minecraft launcher that
**does not require a Microsoft account**, plus the broader "best Minecraft launcher" intent, without
saying anything untrue about what offline accounts can do.

## The one rule on wording
Offline accounts work for **singleplayer, LAN and offline-mode servers**, with Arctic skins, capes,
cosmetics, emotes and friends. Servers that check accounts still need a Microsoft account that owns
Minecraft: Java Edition. Never write "play on any server for free". Keep the existing precise FAQ
sentence as the source of truth and reuse its words.

## What's wrong today
1. **Download page, System requirements.** Reads: "Account: A Microsoft account that owns Minecraft:
   Java Edition for online servers. Offline accounts work for singleplayer, LAN and offline-mode
   servers." The lead is "A Microsoft account", so it reads as a requirement. Change to:
   "Account: None needed to start. A Microsoft account that owns Minecraft: Java Edition for online
   servers; offline accounts for singleplayer, LAN and offline-mode servers."
2. **Homepage `<title>` and meta description** don't mention accounts at all.
   Suggested title: `Arctic Launcher: Free Minecraft Launcher, Microsoft or Offline Accounts`
   (keep under ~60 characters in the visible part; test a shorter variant:
   `Arctic Launcher: Free Minecraft Launcher with Offline Accounts, Skins & Mods`).
   Suggested description: "Free, open-source Minecraft: Java Edition launcher for Windows, macOS and
   Linux. Sign in with Microsoft or play with an offline account (no Microsoft account needed for
   singleplayer, LAN and offline-mode servers). Free skins, capes, cosmetics, Fabric, Forge and
   NeoForge mods, built-in performance mods."
3. **The only strong answer is the 4th FAQ item.** Move "Can I play without a Microsoft account?"
   to first, mark the FAQ up as `FAQPage` JSON-LD so it can appear as a rich result.
4. **No page targets the query.** Searches like "minecraft launcher without microsoft account",
   "minecraft offline launcher", "play minecraft offline mode launcher", "tlauncher alternative" have
   no landing page. The sitemap has `/compare/tlauncher`; there is no `/offline-mode` page.
5. **Download-page title** has a stray non-ASCII character in the title ("Download ? Arctic
   Launcher" renders with a replacement character in some clients). Use a plain hyphen or colon.

## Changes to make
- **New page `/offline-mode`** (add to the sitemap, link from the footer, the homepage FAQ,
  `/download`, `/compare/tlauncher`): H1 "Play Minecraft without a Microsoft account". Sections: what
  works (singleplayer, LAN, offline-mode servers, Arctic skins/capes/cosmetics/friends), what doesn't
  (servers that verify accounts), how to add an offline account (3 steps with a screenshot), safety
  and privacy (keys stay on your PC), links to download. 500+ words, one `FAQPage` block.
- **Homepage hero**: one line under the headline: "Microsoft or offline accounts. No account
  needed to try it."
- **`/compare/tlauncher`**: add a row "Works without a Microsoft account: both. Free skins and capes
  other players see: Arctic."
- **Structured data**: `SoftwareApplication` (name, operatingSystem Windows/macOS/Linux,
  applicationCategory GameApplication, offers price 0, downloadUrl), `FAQPage`, `Organization`.
- **Open Graph / Twitter cards** with a real 1200x630 image on every page (check which pages lack
  one), and unique `<title>` and meta description per page (the `/faq` URL currently 404s but serves
  the homepage's title and description, which is duplicate metadata).
- **Internal links**: every page links to `/download`; docs pages link back to the homepage.
- **Image alt text and sizes**: the homepage HTML is 624 KB; check what is inline (large inline SVG
  or base64), lazy-load images below the fold, and set explicit width and height.
- **Core Web Vitals**: run Lighthouse on `/` and `/download`, fix what it flags (LCP image, layout
  shift from the app screenshot).
- **`robots.txt` / sitemap**: fine as is (AI crawlers allowed). Add `lastmod` to sitemap entries,
  keep the changelog page updated automatically each release.
- **Search Console and Bing Webmaster Tools**: verify the domain, submit the sitemap, watch the
  queries report for "offline" and "without microsoft account" after the new page ships.
- **Changelog / release pages**: each release notes page gets its own title and description
  ("Arctic Launcher 0.4.9: chat history, pack groups, gentle launch").

## Also done on the GitHub side (2026-10-16, by the launcher session)
- README "Your accounts" section now says offline accounts need no Microsoft account.
- Repo description rewritten to mention "Microsoft or offline accounts (no Microsoft account needed)"
  and Forge/NeoForge.
- Topics added: `offline-mode`, `offline-launcher`, `minecraft-mods`, `forge`, `neoforge`
  (20 of 20 used).

## What I did not touch
The website's source is not in this repository, so nothing above is changed on the site yet.
