// 渲染 SVG → 1024 PNG + 各小尺寸 + 程序化质检
const { Resvg } = require("@resvg/resvg-js");
const fs = require("fs");

const svg = fs.readFileSync("app-icon-new.svg", "utf8");

// 1) 1024 母图
const resvg = new Resvg(svg, {
  fitTo: { mode: "width", value: 1024 },
  font: { loadSystemFonts: false },
});
const png = resvg.render().asPng();
fs.writeFileSync("icon-1024.png", png);
console.log("icon-1024.png:", png.length, "bytes");

// 2) 小尺寸渲染（resvg 直接按宽度重栅格化，比 Lanczos 缩小更干净）
for (const size of [256, 64, 32, 24, 16]) {
  const r = new Resvg(svg, {
    fitTo: { mode: "width", value: size },
    font: { loadSystemFonts: false },
  });
  const p = r.render().asPng();
  fs.writeFileSync(`icon-${size}.png`, p);
  console.log(`icon-${size}.png:`, p.length, "bytes");
}

// 3) 程序化质检：读取渲染像素做客观检查
function rawPixels(svgText, size) {
  const r = new Resvg(svgText, {
    fitTo: { mode: "width", value: size },
    font: { loadSystemFonts: false },
    background: "rgba(0,0,0,0)",
  });
  const rendered = r.render();
  return { width: rendered.width, height: rendered.height, data: rendered.pixels };
}

const lum = ([r, g, b]) => 0.2126 * r + 0.7152 * g + 0.0722 * b;

// 母图检查：四角必须透明（圆角正确）、前景主体显著亮于底色
{
  const { width, data } = rawPixels(svg, 1024);
  const px = (x, y) => {
    const i = (y * width + x) * 4;
    return [data[i], data[i + 1], data[i + 2], data[i + 3]];
  };
  console.log("\n[QC] 1024:");
  const corner = px(2, 2);
  console.log("  corner alpha =", corner[3], corner[3] === 0 ? "OK(transparent)" : "FAIL");
  const subject = lum(px(462, 512)); // 主体内（左数第三根条）
  const bg = lum(px(120, 160));      // 底色（贴片内、主体外）
  console.log("  subject lumen =", subject.toFixed(0), " bg lumen =", bg.toFixed(0),
    subject - bg > 80 ? "OK(contrast)" : "WEAK");
}

// 32px 小尺寸检查：主体仍需亮于底色（任务栏/托盘可识别）
{
  const { width, data } = rawPixels(svg, 32);
  const px = (x, y) => {
    const i = (y * width + x) * 4;
    return [data[i], data[i + 1], data[i + 2], data[i + 3]];
  };
  console.log("\n[QC] 32px:");
  const corner = px(1, 1);
  console.log("  corner alpha =", corner[3]);
  const subject = lum(px(14, 16)); // 中间条内
  const bg = lum(px(5, 5));        // 底色
  console.log("  subject lumen =", subject.toFixed(0), "bg lumen =", bg.toFixed(0),
    subject - bg > 60 ? "OK(contrast)" : "WEAK");
}

console.log("\ndone");
