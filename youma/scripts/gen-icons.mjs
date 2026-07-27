// Generates all Tauri-required icon files from youma-source.svg
import sharp from 'sharp';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const iconsDir = path.join(__dirname, '..', 'src-tauri', 'icons');
const svgPath = path.join(iconsDir, 'youma-source.png');
const svgBuf = fs.readFileSync(svgPath);

async function png(size, outName) {
  const dest = path.join(iconsDir, outName);
  await sharp(svgBuf).resize(size, size).png().toFile(dest);
  console.log(`  ✓ ${outName} (${size}x${size})`);
}

// Build a minimal multi-size ICO (16, 32, 48, 256) by hand
async function ico(sizes, outName) {
  const images = await Promise.all(
    sizes.map(s => sharp(svgBuf).resize(s, s).png().toBuffer())
  );

  // ICO header
  const count = images.length;
  const headerSize = 6 + count * 16;
  let offset = headerSize;
  const entries = [];
  for (let i = 0; i < images.length; i++) {
    const s = sizes[i];
    entries.push({ buf: images[i], size: s, offset });
    offset += images[i].length;
  }

  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);   // reserved
  header.writeUInt16LE(1, 2);   // type: ICO
  header.writeUInt16LE(count, 4);

  const dirEntries = Buffer.alloc(count * 16);
  entries.forEach(({ buf, size, offset: off }, i) => {
    const base = i * 16;
    dirEntries.writeUInt8(size >= 256 ? 0 : size, base);      // width (0 = 256)
    dirEntries.writeUInt8(size >= 256 ? 0 : size, base + 1);  // height
    dirEntries.writeUInt8(0, base + 2);   // color count
    dirEntries.writeUInt8(0, base + 3);   // reserved
    dirEntries.writeUInt16LE(1, base + 4); // planes
    dirEntries.writeUInt16LE(32, base + 6); // bit count
    dirEntries.writeUInt32LE(buf.length, base + 8);
    dirEntries.writeUInt32LE(off, base + 12);
  });

  const dest = path.join(iconsDir, outName);
  fs.writeFileSync(dest, Buffer.concat([header, dirEntries, ...entries.map(e => e.buf)]));
  console.log(`  ✓ ${outName} (${sizes.join('+')})`);
}

console.log('Generating icons...');
await png(32, '32x32.png');
await png(64, '64x64.png');
await png(128, '128x128.png');
await png(256, '128x128@2x.png');
await png(512, 'icon.png');
await png(30,  'Square30x30Logo.png');
await png(44,  'Square44x44Logo.png');
await png(71,  'Square71x71Logo.png');
await png(89,  'Square89x89Logo.png');
await png(107, 'Square107x107Logo.png');
await png(142, 'Square142x142Logo.png');
await png(150, 'Square150x150Logo.png');
await png(284, 'Square284x284Logo.png');
await png(310, 'Square310x310Logo.png');
await png(50,  'StoreLogo.png');
await ico([16, 24, 32, 48, 64, 128, 256], 'icon.ico');

// macOS .icns — write as a 512-px PNG named correctly; Tauri bundles it
// (proper ICNS requires platform tooling; Tauri CLI rebuilds it on macOS)
await png(512, 'icon.icns.png');
const src512 = path.join(iconsDir, 'icon.icns.png');
const dst    = path.join(iconsDir, 'icon.icns');
if (!fs.existsSync(dst)) {
  fs.copyFileSync(src512, dst);
  console.log('  ✓ icon.icns (placeholder — rebuild on macOS with tauri icon)');
} else {
  console.log('  ℹ icon.icns exists, skipping (rebuild on macOS if needed)');
}
fs.rmSync(src512);

console.log('Done.');
