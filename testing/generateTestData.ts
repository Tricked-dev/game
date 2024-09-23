// spawn 2 instances of bot.ts file using workers
import { Worker } from "worker_threads";
import path from "path";

const spawnWorker = (filePath: string) => {
  return new Promise<void>((resolve, reject) => {
    const worker = new Worker(filePath);

    worker.on("message", resolve);
    worker.on("error", reject);
    worker.on("exit", (code) => {
      if (code !== 0) {
        reject(new Error(`Worker stopped with exit code ${code}`));
      }
    });
  });
};

const run = async () => {
  const botPath = path.resolve(__dirname, "bot.ts");

  try {
    let count = 2;
    let workers = [];
    for (let i = 0; i < count; i++) {
      workers.push(spawnWorker(botPath));
    }
    Promise.all(workers);
    console.log("Both bots spawned successfully.");
  } catch (error) {
    console.error("Error spawning bots:", error);
  }
};

run();
