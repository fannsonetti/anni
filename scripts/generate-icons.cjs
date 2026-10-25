// Run with Node and the sharp package installed. Build/release uses the committed assets.
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');

(async () => {
    const assets = path.join(__dirname, '../crates/inna-app/assets');
    const source = fs.readFileSync(path.join(assets, 'anni.svg'));
    await sharp(source).resize(256, 256).png().toFile(path.join(assets, 'anni.png'));
    const sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256];
    const images = await Promise.all(sizes.map(size => sharp(source).resize(size, size).png().toBuffer()));
    const header = Buffer.alloc(6 + sizes.length * 16);
    header.writeUInt16LE(1, 2);
    header.writeUInt16LE(sizes.length, 4);
    let offset = header.length;
    sizes.forEach((size, i) => {
        const entry = 6 + i * 16;
        header[entry] = header[entry + 1] = size === 256 ? 0 : size;
        header.writeUInt16LE(1, entry + 4);
        header.writeUInt16LE(32, entry + 6);
        header.writeUInt32LE(images[i].length, entry + 8);
        header.writeUInt32LE(offset, entry + 12);
        offset += images[i].length;
    });
    fs.writeFileSync(path.join(assets, 'anni.ico'), Buffer.concat([header, ...images]));
})().catch(error => { console.error(error); process.exitCode = 1; });
