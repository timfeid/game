export async function POST(req) {
	const { refresh_token } = await req.request.json();
	req.cookies.set('refresh_token', refresh_token, { path: '/', secure: false });

	return new Response(null, { status: 201 });
}
