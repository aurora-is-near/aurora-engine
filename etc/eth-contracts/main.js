const fs = require('fs');
const path = require('path');
const artifact = require(process.argv[2]);
if (!fs.existsSync('res')) {
    fs.mkdirSync('res');
}
fs.writeFileSync(path.join('res', 'EvmErc20.bin'), artifact.bytecode.substring(2));