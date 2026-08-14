<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useUpdater } from "../composables/useUpdater";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ close: [] }>();

const updater = useUpdater();
const hasChecked = ref(false);

const GITHUB_URL = "https://github.com/sajjadbzrn";
const GITHUB_AVATAR = "https://avatars.githubusercontent.com/u/292075678?v=4";
const CREATOR_NAME = "Sajjad";
const CREATOR_HANDLE = "sajjadbzrn";
const CREATOR_BIO = "Full-stack engineer · TypeScript / Bun / Cloudflare Workers";

const version = ref("0.1.0");
const avatarFailed = ref(false);

function onKeydown(e: KeyboardEvent): void {
  if (e.key === "Escape" && props.open) emit("close");
}

async function checkForUpdates(): Promise<void> {
  await updater.checkForUpdates();
  hasChecked.value = true;
}

function formatDate(d: Date | string | null | undefined): string {
  if (!d) return "";
  const date = typeof d === "string" ? new Date(d) : d;
  return date.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      avatarFailed.value = false;
      getVersion()
        .then((v) => (version.value = v))
        .catch(() => {});
      // Auto-check in the background the first time the modal is opened.
      if (!updater.updateAvailable.value && !updater.checking.value && !hasChecked.value) {
        void checkForUpdates();
      }
    }
  },
);

onMounted(() => window.addEventListener("keydown", onKeydown));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));

async function openGithub(): Promise<void> {
  await openUrl(GITHUB_URL);
}
</script>

<template>
  <Teleport to="body">
    <Transition name="modal">
      <div v-if="open" class="backdrop" @click.self="emit('close')">
        <div class="card" role="dialog" aria-modal="true" aria-label="About FilePressor">
          <button class="close" type="button" title="Close" @click="emit('close')">
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
              <path d="M6 6l12 12M18 6L6 18" />
            </svg>
          </button>

          <!-- Hero -->
          <header class="hero">
            <span class="logo" aria-hidden="true">
              <svg viewBox="0 0 48 48" width="58" height="58">
                <defs>
                  <linearGradient id="fp-grad-lg" x1="0" y1="0" x2="1" y2="1">
                    <stop offset="0" stop-color="#C04D6F" />
                    <stop offset="1" stop-color="#E08BA4" />
                  </linearGradient>
                </defs>
                <rect x="9" y="8" width="28" height="33" rx="7" fill="url(#fp-grad-lg)" />
                <rect x="15" y="15" width="18" height="3.4" rx="1.7" fill="#ffffff" opacity="0.92" />
                <rect x="15" y="22" width="18" height="3.4" rx="1.7" fill="#ffffff" opacity="0.74" />
                <rect x="15" y="29" width="11" height="3.4" rx="1.7" fill="#ffffff" opacity="0.6" />
              </svg>
            </span>
            <div class="hero-text">
              <h2 class="name">FilePressor</h2>
              <span class="version">v{{ version }}</span>
            </div>
            <p class="tagline">Compress anything · open anything — all offline, no extra apps.</p>
            <div class="stack">
              <span>Rust</span><span>zstd</span><span>LZMA2</span><span>7Z</span><span>RAR</span><span>Tauri</span>
            </div>
          </header>

          <p class="blurb">
            A fast, lightweight desktop compressor. Shrink files, re-compress existing
            archives, and open almost any archive format — entirely on your machine.
          </p>

          <div class="divider"></div>

          <!-- Updates -->
          <section class="updates">
            <div class="upd-head">
              <span class="upd-title">Updates</span>
              <label class="upd-auto">
                <input
                  type="checkbox"
                  :checked="updater.autoCheck.value"
                  @change="updater.setAutoCheck(($event.target as HTMLInputElement).checked)"
                />
                <span>Check automatically on launch</span>
              </label>
            </div>

            <div class="upd-row">
              <span class="upd-version">v{{ version }}</span>
              <button
                class="upd-check"
                type="button"
                :disabled="updater.checking.value || updater.downloading.value"
                @click="checkForUpdates"
              >
                {{ updater.checking.value ? "Checking…" : "Check for updates" }}
              </button>
            </div>

            <p v-if="updater.error.value" class="upd-msg err">{{ updater.error.value }}</p>
            <template v-else-if="updater.updateAvailable.value && updater.update.value">
              <p class="upd-msg">
                <strong>v{{ updater.update.value.version }}</strong> is available
                <span v-if="updater.update.value.date"> · {{ formatDate(updater.update.value.date) }}</span>
              </p>
              <p v-if="updater.update.value.body" class="upd-notes">{{ updater.update.value.body }}</p>
              <button
                class="upd-update"
                type="button"
                :disabled="updater.downloading.value"
                @click="updater.installUpdate"
              >
                {{ updater.downloading.value ? "Updating…" : "Download & install update" }}
              </button>
              <div v-if="updater.downloading.value" class="upd-track">
                <div
                  class="upd-fill"
                  :style="{ width: updater.contentLength.value ? ((updater.downloaded.value / updater.contentLength.value) * 100) + '%' : '30%' }"
                ></div>
              </div>
            </template>
            <p
              v-else-if="hasChecked && !updater.checking.value"
              class="upd-msg ok"
            >
              You're on the latest version.
            </p>
          </section>

          <div class="divider"></div>

          <!-- Creator -->
          <section class="creator">
            <div class="avatar-wrap">
              <img
                v-if="!avatarFailed"
                class="avatar"
                :src="GITHUB_AVATAR"
                alt="Sajjad"
                referrerpolicy="no-referrer"
                @error="avatarFailed = true"
              />
              <span v-else class="avatar avatar-fallback">{{ CREATOR_NAME[0] }}</span>
            </div>
            <div class="creator-info">
              <span class="creator-name">{{ CREATOR_NAME }}</span>
              <span class="creator-handle">@{{ CREATOR_HANDLE }}</span>
              <span class="creator-bio">{{ CREATOR_BIO }}</span>
            </div>
            <button class="github-btn" type="button" @click="openGithub">
              <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                <path d="M12 .5C5.65.5.5 5.65.5 12c0 5.08 3.29 9.39 7.86 10.91.58.11.79-.25.79-.55 0-.27-.01-1.17-.02-2.12-3.2.7-3.87-1.36-3.87-1.36-.52-1.33-1.28-1.68-1.28-1.68-1.04-.71.08-.7.08-.7 1.15.08 1.76 1.18 1.76 1.18 1.03 1.76 2.69 1.25 3.35.96.1-.75.4-1.25.72-1.54-2.55-.29-5.24-1.28-5.24-5.68 0-1.26.45-2.28 1.18-3.09-.12-.29-.51-1.46.11-3.04 0 0 .96-.31 3.15 1.18a10.9 10.9 0 0 1 5.74 0c2.19-1.49 3.15-1.18 3.15-1.18.62 1.58.23 2.75.11 3.04.73.81 1.18 1.83 1.18 3.09 0 4.41-2.69 5.38-5.25 5.67.41.35.77 1.05.77 2.12 0 1.53-.01 2.76-.01 3.14 0 .3.2.66.8.55A11.52 11.52 0 0 0 23.5 12C23.5 5.65 18.35.5 12 .5z" />
              </svg>
              GitHub
            </button>
          </section>

          <p class="foot">Created by {{ CREATOR_NAME }} · built with Rust, Tauri &amp; Vue</p>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(20, 14, 18, 0.45);
  backdrop-filter: blur(6px);
  -webkit-backdrop-filter: blur(6px);
}

.card {
  position: relative;
  width: 420px;
  max-width: 100%;
  max-height: 90vh;
  overflow-y: auto;
  padding: 26px 26px 20px;
  border-radius: 24px;
  background: var(--surface, #fff);
  border: 1px solid var(--border, #ece8ee);
  box-shadow: 0 24px 60px -20px rgba(0, 0, 0, 0.35);
  text-align: center;
}

.close {
  position: absolute;
  top: 14px;
  right: 14px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border: none;
  border-radius: 10px;
  background: transparent;
  color: var(--text-muted, #8a8090);
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}

.close:hover {
  background: var(--brand-soft, #f8e9ee);
  color: var(--brand-strong, #a93a5e);
}

/* ---------- Hero ---------- */
.hero {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.logo {
  display: inline-flex;
  filter: drop-shadow(0 8px 18px rgba(192, 77, 111, 0.4));
  margin-bottom: 12px;
}

.hero-text {
  display: flex;
  align-items: baseline;
  gap: 9px;
}

.name {
  margin: 0;
  font-size: 23px;
  font-weight: 800;
  letter-spacing: -0.02em;
  color: var(--text, #211b20);
}

.version {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
  padding: 3px 10px;
  border-radius: 999px;
  color: var(--brand, #c04d6f);
  background: var(--brand-soft, #f8e9ee);
}

.tagline {
  margin: 10px 0 0;
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text-muted, #8a8090);
}

.stack {
  display: flex;
  justify-content: center;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 14px;
}

.stack span {
  font-size: 10.5px;
  font-weight: 650;
  letter-spacing: 0.03em;
  padding: 4px 10px;
  border-radius: 999px;
  border: 1px solid var(--border, #ece8ee);
  color: var(--text-muted, #8a8090);
}

.blurb {
  margin: 18px 0 0;
  font-size: 12.5px;
  line-height: 1.6;
  color: var(--text-muted, #8a8090);
}

/* ---------- Divider ---------- */
.divider {
  height: 1px;
  margin: 20px 0 16px;
  background: var(--border, #ece8ee);
}

/* ---------- Creator ---------- */
.creator {
  display: flex;
  align-items: center;
  gap: 13px;
  text-align: left;
  padding: 12px;
  border-radius: 16px;
  background: var(--seg-bg, #f1edf2);
  border: 1px solid var(--border, #ece8ee);
}

.avatar-wrap {
  flex-shrink: 0;
}

.avatar {
  width: 48px;
  height: 48px;
  border-radius: 50%;
  border: 2px solid var(--brand, #c04d6f);
  object-fit: cover;
  background: var(--brand-soft, #f8e9ee);
}

.avatar-fallback {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 20px;
  font-weight: 800;
  color: var(--brand, #c04d6f);
}

.creator-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.creator-name {
  font-size: 14px;
  font-weight: 700;
  color: var(--text, #211b20);
}

.creator-handle {
  font-size: 11.5px;
  font-weight: 600;
  color: var(--brand, #c04d6f);
}

.creator-bio {
  font-size: 11px;
  color: var(--text-muted, #8a8090);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.github-btn {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 8px 14px;
  border: none;
  border-radius: 11px;
  font-size: 12.5px;
  font-weight: 650;
  font-family: inherit;
  color: #fff;
  background: linear-gradient(135deg, var(--brand), #d98ba3);
  cursor: pointer;
  transition: filter 0.15s ease, transform 0.12s ease, box-shadow 0.15s ease;
  box-shadow: 0 6px 16px -6px var(--brand-glow);
}

.github-btn:hover {
  filter: brightness(1.06);
  transform: translateY(-1px);
}

.foot {
  margin: 16px 0 0;
  font-size: 11px;
  color: var(--text-muted, #8a8090);
  opacity: 0.85;
}

/* ---------- Updates ---------- */
.updates {
  display: flex;
  flex-direction: column;
  gap: 12px;
  text-align: left;
}

.upd-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  flex-wrap: wrap;
}

.upd-title {
  font-size: 13px;
  font-weight: 700;
  color: var(--text, #211b20);
}

.upd-auto {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: 11.5px;
  color: var(--text-muted, #8a8090);
  cursor: pointer;
}

.upd-auto input {
  width: 15px;
  height: 15px;
  accent-color: var(--brand, #c04d6f);
  cursor: pointer;
}

.upd-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.upd-version {
  font-size: 12.5px;
  font-weight: 650;
  color: var(--text-muted, #8a8090);
}

.upd-check {
  border: 1px solid var(--border, #ece8ee);
  background: var(--surface, #fff);
  color: var(--text, #211b20);
  border-radius: 10px;
  padding: 7px 14px;
  font-family: inherit;
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
  transition: border-color 0.15s ease, color 0.15s ease;
}

.upd-check:hover:not(:disabled) {
  border-color: var(--brand, #c04d6f);
  color: var(--brand, #c04d6f);
}

.upd-check:disabled {
  opacity: 0.6;
  cursor: default;
}

.upd-msg {
  margin: 0;
  font-size: 12px;
  color: var(--text-muted, #8a8090);
}

.upd-msg.ok {
  color: #2e8b57;
}

.upd-msg.err {
  color: #c0392b;
  word-break: break-word;
}

.upd-notes {
  margin: 0;
  font-size: 11.5px;
  line-height: 1.5;
  white-space: pre-wrap;
  color: var(--text-muted, #8a8090);
  max-height: 96px;
  overflow-y: auto;
}

.upd-update {
  align-self: flex-start;
  border: none;
  border-radius: 10px;
  padding: 8px 16px;
  font-family: inherit;
  font-size: 12.5px;
  font-weight: 650;
  color: #fff;
  background: linear-gradient(135deg, var(--brand, #c04d6f), #d98ba3);
  cursor: pointer;
  box-shadow: 0 6px 16px -6px var(--brand-glow);
  transition: filter 0.15s ease;
}

.upd-update:hover:not(:disabled) {
  filter: brightness(1.07);
}

.upd-update:disabled {
  opacity: 0.7;
  cursor: default;
}

.upd-track {
  width: 100%;
  height: 6px;
  border-radius: 999px;
  background: var(--seg-bg, #f1edf2);
  overflow: hidden;
}

.upd-fill {
  height: 100%;
  border-radius: 999px;
  background: var(--brand, #c04d6f);
  transition: width 0.25s ease;
}

/* transitions */
.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.18s ease;
}

.modal-enter-active .card,
.modal-leave-active .card {
  transition: transform 0.22s cubic-bezier(0.2, 0.9, 0.3, 1.15), opacity 0.18s ease;
}

.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

.modal-enter-from .card,
.modal-leave-to .card {
  opacity: 0;
  transform: translateY(10px) scale(0.96);
}
</style>
