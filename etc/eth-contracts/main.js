const fs = require('fs');
const path = require('path');
const artifact = require(process.argv[2]);
if (!fs.existsSync('res')) {
    fs.mkdirSync('res');
}
const contractName = artifact.contractName;

// The engine embeds these files with `include_bytes!`, so rewriting them unchanged would still
// make Cargo rebuild the engine and everything that depends on it.
function writeIfChanged (file, contents) {
    if (fs.existsSync(file) && fs.readFileSync(file).equals(contents)) {
        return;
    }

    fs.writeFileSync(file, contents);
}

writeIfChanged(path.join('res', contractName + '.hex'), Buffer.from(artifact.bytecode));
writeIfChanged(path.join('res', contractName + '.bin'), Buffer.from(artifact.bytecode.substring(2), 'hex'));
