import puppeteer from "puppeteer-core";

const spawnBot = async () => {
  const browser = await puppeteer.launch({
    executablePath: "/usr/bin/brave-beta",
    headless: false,
    args: ["--no-sandbox"],
    defaultViewport: { width: 1080, height: 1024 },
  });
  const page = (await browser.pages())[0] || (await browser.newPage());

  await page.goto("http://localhost:4321");
  await page.setViewport({ width: 1080, height: 1024 });
  await page.evaluate(() => localStorage.setItem("autoplay", "true"));
  await page.reload();
  // setInterval(
  //   async () => {
  //     await page.reload();
  //     await page.goto("http://localhost:4321");
  //   },
  //   Math.random() * (1000 * 120),
  // );

  // setInterval(
  //   async () => {
  //     await localStorage.removeItem("userInfo");
  //     await page.reload();
  //   },
  //   Math.random() * (1000 * 600),
  // );
};
await spawnBot();
