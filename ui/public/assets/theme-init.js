(() => {
  const stored = localStorage.getItem("oxray.theme");
  const preference =
    stored === "dark" || stored === "light" ? stored : "system";
  const resolved =
    preference === "system"
      ? window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light"
      : preference;
  document.documentElement.setAttribute("data-theme", resolved);
})();
