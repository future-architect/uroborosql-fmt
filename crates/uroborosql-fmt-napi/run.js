const fs = require("fs");

const { runfmt } = require("./index.js");

let target = fs.readFileSync("./test.sql", "utf-8");
try {
  const startTime = performance.now();
  let res = runfmt(target, null);
  console.log(res);
  const endTime = performance.now();
  console.log(`format complete: ${endTime - startTime} ms`);
} catch (e) {
  console.log(e);
}
