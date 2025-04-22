import { useRef, useEffect, useState } from 'react';
import { API_ENDPOINTS } from '../config/api';
import '../BlockStream.css';

interface BlockAggregate {
    date: string;
    block_height: number;
    block_hash_big_endian: string;
    total_utxos: number;
    total_sats: number;
  }

function BlockVisualizer() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [blocks, setBlocks] = useState<BlockAggregate[]>([]);

  useEffect(() => {
    
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    let x = 0;

    const drawBlock = (block: BlockAggregate) => {

            ctx.clearRect(0, 0, canvas.width, canvas.height);
            ctx.fillStyle = 'green';

            /*
                x: horizontal position of the rectangle's top-left corner
                y: vertical position of the rectangle's top-left corner
                width: width of the rectangle
                height: height of the rectangle
             */
            ctx.fillRect(x, 0, 400, 215);
    
            // Set text properties
            ctx.fillStyle = 'white'; // Text color
            ctx.font = '20px Arial'; // Font size and family
            ctx.textAlign = 'left'; // Center the text horizontally
            ctx.textBaseline = 'middle'; // Center the text vertically
    
            // Display block height in the center of the block
            ctx.fillText("Block Height:  " + block.block_height, x + 5, 55);
            ctx.fillText("Total Aggregate UTXOs:  " + block.total_utxos, x + 5, 105);
            ctx.fillText("Total Aggregate Value (BTC):  " + (block.total_sats / 100000000).toFixed(2), x + 5, 155);
    };
    

    const eventSource = new EventSource(API_ENDPOINTS.blockStream);

    eventSource.onopen = () => {
      console.log('Connected to SSE server successfully.');
    };

    eventSource.onmessage = (event) => {
      const newBlock = JSON.parse(event.data) as BlockAggregate;
      //console.log('New block received:', newBlock);
      drawBlock(newBlock);
    };

    eventSource.onerror = (error) => {
      console.error('Error with SSE connection:', error);
    };

    return () => {
      eventSource.close();
    };


  }, []);

  return (
    <div className="block-visualizer">
        <h2 className="text-xl font-bold mb-4">Live P2PK Block Aggregate Stream</h2>

        <div className="block-visualizer-canvas">
            {/*
                The canvas is the element that will be used to draw the block aggregate stream.
                Any drawing operations will be performed within this canvas element.
            */}
            <canvas ref={canvasRef} width={800} height={250} />
        </div>
    </div>
  );
}

export default BlockVisualizer;