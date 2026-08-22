import http from 'k6/http';
import { check, sleep } from 'k6';

const baseUrl = __ENV.NSTREAM_API_URL || 'http://localhost:8080';
const headers = {
  'Content-Type': 'application/json',
  ...( __ENV.NSTREAM_API_KEY ? { 'X-API-Key': __ENV.NSTREAM_API_KEY } : {}),
};

export const options = {
  scenarios: {
    api: { executor: 'ramping-arrival-rate', startRate: 2, timeUnit: '1s', preAllocatedVUs: 20,
      stages: [{ target: 10, duration: '1m' }, { target: 30, duration: '3m' }, { target: 0, duration: '30s' }] },
  },
  thresholds: { http_req_failed: ['rate<0.01'], http_req_duration: ['p(95)<500'] },
};

export default function () {
  const health = http.get(`${baseUrl}/health`);
  check(health, { 'health is 200': (r) => r.status === 200 });
  const jobs = http.get(`${baseUrl}/jobs`, { headers });
  check(jobs, { 'list jobs is 200': (r) => r.status === 200 });
  sleep(1);
}
