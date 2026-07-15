export class EggfetchClient {
  constructor();
  get(url: string): Promise<EggfetchResponse>;
  post(url: string, body?: string): Promise<EggfetchResponse>;
  put(url: string, body?: string): Promise<EggfetchResponse>;
  patch(url: string, body?: string): Promise<EggfetchResponse>;
  delete(url: string): Promise<EggfetchResponse>;
  head(url: string): Promise<EggfetchResponse>;
  options(url: string): Promise<EggfetchResponse>;
  request(method: string, url: string, body?: string): Promise<EggfetchResponse>;
}

export class EggfetchResponse {
  readonly status: number;
  readonly url: string;
  readonly text: string;
  readonly json: any;
  readonly headers: Record<string, string>;
  readonly ok: boolean;
}
