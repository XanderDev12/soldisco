import { proxyLocalApiRequest } from "@/app/lib/soldisco-api/localProxy";

type LocalApiRouteContext = {
  params: Promise<{
    port: string;
    path: string[];
  }>;
};

async function handle(
  request: Request,
  context: LocalApiRouteContext,
): Promise<Response> {
  const { port, path } = await context.params;
  return proxyLocalApiRequest(request, port, path);
}

export const dynamic = "force-dynamic";

export function GET(
  request: Request,
  context: LocalApiRouteContext,
): Promise<Response> {
  return handle(request, context);
}

export function POST(
  request: Request,
  context: LocalApiRouteContext,
): Promise<Response> {
  return handle(request, context);
}

export function PUT(
  request: Request,
  context: LocalApiRouteContext,
): Promise<Response> {
  return handle(request, context);
}
