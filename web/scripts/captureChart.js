   // captureChart.js
   const puppeteer = require('puppeteer');

   (async () => {
     // Retrieve the block_num from command-line arguments, if provided
     const args = process.argv.slice(2);
     const blockNum = args.length > 0 ? args[0] : "LATEST";

     const browser = await puppeteer.launch({ 
         headless: true,
         args: ['--no-sandbox', '--disable-setuid-sandbox']
     });
     const page = await browser.newPage();

     // Gabriel running in dev mode:  http://0.0.0.0:3001
     // Gabriel running in release mode: http://0.0.0.0:3000
     const reactAppSocketAddr = process.env.GABRIEL_REACT_APP_BASE_URL || 'http://0.0.0.0:3000';
     console.log(`captureChart.js: Navigating to ${reactAppSocketAddr}`);

     // Log console messages
     //page.on('console', msg => console.log('PAGE LOG :', msg.text()));

     // Monitor network requests
     page.on('requestfinished', request => {
       //console.log('captureChart.js: Request finished:', request.url());
     });

     // Load your chart page
     await page.goto(`${reactAppSocketAddr}`);
     //console.log('captureChart.js: Navigated to p2pk-blocks-graph');

     // Wait for the chart to render
     await page.waitForSelector('#chart-container');
     //console.log('captureChart.js: Chart container found');
     // Wait for a specific element or text that indicates data is loaded
     await page.waitForFunction(() => {
       const chartContainer = document.querySelector('#chart-container');
       return chartContainer && chartContainer.innerText.includes('Total UTXOs');
     });
     //console.log('captureChart.js: Waiting for data to load');

     // Wait for a configurable amount of time to allow the dynamic data to render
     const wait_time_seconds = process.env.CHART_CAPTURE_DELAY_SECONDS || 10;
     await new Promise(resolve => setTimeout(resolve, wait_time_seconds * 1000));
     //console.log('captureChart.js: Data loaded');

     // Get IMAGE_DIR_PATH from environment variable; Default to /tmp/gabriel/images
     const imageDirPath = process.env.CHART_CAPTURE_IMAGE_DIR_PATH || '/tmp/gabriel/images';

     // Capture the chart as an image
     const chartElement = await page.$('#chart-container');
     await chartElement.screenshot({ path: `${imageDirPath}/p2pk_chart_${blockNum}.png` });
     console.log(`Chart captured and written to ${imageDirPath}/p2pk_chart_${blockNum}.png`);
     await browser.close();
   })();
