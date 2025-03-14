// Determine the base URL based on the environment
const API_BASE_URL = process.env.NODE_ENV === 'development'
  ? 'http://localhost:3000' // Development server URL
  : ''; // Empty string for production to use relative paths

console.log("API_BASE_URL: ", API_BASE_URL)

// You could also add other API-related configuration here
export const API_ENDPOINTS = {
    latestBlocks: `${API_BASE_URL}/api/blocks/latest`,
    blockByHash: (hash: string) => `${API_BASE_URL}/api/block/hash/${hash}`,
    blockByHeight: (height: number) => `${API_BASE_URL}/api/block/height/${height}`,
    blockStream: `${API_BASE_URL}/api/blocks/stream`,
};


