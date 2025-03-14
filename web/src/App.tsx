import React from 'react';
import { BrowserRouter as Router, Route, Routes } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import P2PKBlocksGraph from './components/P2PKBlocksGraph';
import BlockVisualizer from './components/BlockVisualizer';
import './App.css';

const queryClient = new QueryClient();

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <Router>
        <div className="container mx-auto p-4">
          <img 
            src="/surmount_logo.png" 
            alt="Bitcoin P2PK Analysis Logo" 
            className="mb-4"
            style={{ 
              margin: '0 auto',
              width: '4%',
              display: 'block'
            }}
          />
          <h1 
            className="text-2xl font-bold mb-4" 
            style={{ 
              margin: '0 auto',
              textAlign: 'center'
            }}
          >
            Bitcoin UTXO Analysis
          </h1>
          <Routes>
            <Route path="/p2pk-blocks-graph" element={<P2PKBlocksGraph />} />
            <Route path="/" element={
              <div 
                className="grid gap-4" 
                style={{ 
                  marginLeft: '48px',
                  marginTop: '42px'
                }}
              >
                <P2PKBlocksGraph />
                <BlockVisualizer />
              </div>
            } />
          </Routes>
        </div>
      </Router>
    </QueryClientProvider>
  );
}

export default App;
