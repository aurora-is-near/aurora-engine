task("storageLayout", "automatically generates the contract storage layout")
  .setAction(async () => {
    await hre.storageLayout.export();
  });

module.exports = {};