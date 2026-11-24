const THEME_KEY = "theme";
const THEME_KEYS = [THEME_KEY, "oni-theme", "color-theme"];

const header = document.querySelector(".site-header");
const nav = document.querySelector(".nav");
const navToggle = document.querySelector("#nav-toggle");
const themeToggle = document.querySelector("#theme-toggle");
const year = document.querySelector("#year");

if (year) {
  year.textContent = String(new Date().getFullYear());
}

function readStoredTheme() {
  for (const key of THEME_KEYS) {
    try {
      const value = localStorage.getItem(key);
      if (value === "light" || value === "dark") return value;
    } catch {
      // localStorage can be unavailable.
    }
  }
  return null;
}

function preferredTheme() {
  const stored = readStoredTheme();
  if (stored) return stored;
  if (
    window.matchMedia &&
    window.matchMedia("(prefers-color-scheme: light)").matches
  ) {
    return "light";
  }
  return "dark";
}

function applyTheme(theme) {
  document.documentElement.setAttribute("data-theme", theme);
  if (!themeToggle) return;
  const next = theme === "dark" ? "light" : "dark";
  themeToggle.setAttribute("aria-label", `Switch to ${next} mode`);
  themeToggle.setAttribute("aria-pressed", String(theme === "light"));
  const glyph = themeToggle.querySelector(".theme-toggle-glyph");
  if (glyph) glyph.textContent = theme === "dark" ? "☾" : "☀";
}

applyTheme(preferredTheme());

if (themeToggle) {
  themeToggle.addEventListener("click", () => {
    const current =
      document.documentElement.getAttribute("data-theme") || "dark";
    const next = current === "dark" ? "light" : "dark";
    try {
      localStorage.setItem(THEME_KEY, next);
    } catch {
      // Ignore storage failures.
    }
    applyTheme(next);
  });
}

if (window.matchMedia) {
  const query = window.matchMedia("(prefers-color-scheme: light)");
  const syncSystem = (event) => {
    let stored = null;
    try {
      stored = localStorage.getItem(THEME_KEY);
    } catch {
      stored = null;
    }
    if (stored !== "light" && stored !== "dark") {
      applyTheme(event.matches ? "light" : "dark");
    }
  };
  if (typeof query.addEventListener === "function") {
    query.addEventListener("change", syncSystem);
  } else if (typeof query.addListener === "function") {
    query.addListener(syncSystem);
  }
}

if (navToggle && nav) {
  navToggle.addEventListener("click", () => {
    const open = nav.classList.toggle("is-open");
    navToggle.setAttribute("aria-expanded", String(open));
  });

  nav.addEventListener("click", (event) => {
    if (event.target instanceof HTMLAnchorElement) {
      nav.classList.remove("is-open");
      navToggle.setAttribute("aria-expanded", "false");
    }
  });
}

if (header) {
  const syncHeader = () => {
    header.classList.toggle("is-scrolled", window.scrollY > 8);
  };
  syncHeader();
  window.addEventListener("scroll", syncHeader, { passive: true });
}

const revealItems = document.querySelectorAll(".reveal");

if ("IntersectionObserver" in window) {
  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          entry.target.classList.add("is-visible");
          observer.unobserve(entry.target);
        }
      }
    },
    { rootMargin: "0px 0px -8% 0px", threshold: 0.1 },
  );

  revealItems.forEach((item) => observer.observe(item));
} else {
  revealItems.forEach((item) => item.classList.add("is-visible"));
}
