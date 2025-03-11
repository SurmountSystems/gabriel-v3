   // captureChart.js
   const puppeteer = require('puppeteer');

   (async () => {
     const browser = await puppeteer.launch({ headless: false });
     const page = await browser.newPage();

     // Log console messages
     page.on('console', msg => console.log('PAGE LOG:', msg.text()));

     // Monitor network requests
     page.on('requestfinished', request => {
       console.log('Request finished:', request.url());
     });

     // Load your chart page
     await page.goto('http://localhost:3001/p2pk-blocks-graph');

     // Wait for the chart to render
     await page.waitForSelector('#chart-container');

     // Wait for a specific element or text that indicates data is loaded
     await page.waitForFunction(() => {
       const chartContainer = document.querySelector('#chart-container');
       return chartContainer && chartContainer.innerText.includes('Total UTXOs');
     });

     // Wait for a configurable amount of time to allow the dynamic data to render
     const wait_time_seconds = process.env.WAIT_TIME_SECONDS || 10;
     await new Promise(resolve => setTimeout(resolve, wait_time_seconds * 1000));

     // Capture the chart as an image
     const chartElement = await page.$('#chart-container');
     await chartElement.screenshot({ path: 'chart.png' });

     await browser.close();
   })();
