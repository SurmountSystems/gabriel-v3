import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import BlocksGraph from './components/BlocksGraph';
import BlockStream from './components/BlockStream';
import './App.css';

const queryClient = new QueryClient();

function App() {
  return (
    <>
      <QueryClientProvider client={queryClient}>
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
          <div 
            className="grid gap-4" 
            style={{ 
              marginLeft: '48px',
              marginTop: '42px'
            }}
          >
            <BlocksGraph />
            <BlockStream />
          </div>
        </div>
      </QueryClientProvider>
    </>
  );
}

export default App;
