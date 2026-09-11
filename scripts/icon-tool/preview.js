// 渲染候选方案预览：1024 / 64 / 32 / 16
const { Resvg } = require("@resvg/resvg-js");
const fs = require("fs");

const names = process.argv.slice(2).length
  ? process.argv.slice(2)
  : ["candidate-a", "candidate-b"];

for (const name of names) {
  const svg = fs.readFileSync(`${name}.svg`, "utf8");
  for (const size of [1024, 64, 32, 16]) {
    const png = new Resvg(svg, {
      fitTo: { mode: "width", value: size },
      font: { loadSystemFonts: false },
    }).render().asPng();
    fs.writeFileSync(`preview-${name}-${size}.png`, png);
    console.log(`preview-${name}-${size}.png`, png.length, "bytes");
  }
}
