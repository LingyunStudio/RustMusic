try {
  var t = localStorage.getItem("rustmusic.theme");
  var bg = t === "dark" ? "#0b0b10" : "#f3f5f8";
  document.documentElement.style.background = bg;
} catch (e) {}
