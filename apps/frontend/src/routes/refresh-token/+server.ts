import { refreshAccessToken } from '../../lib/mutations/auth.js';

export async function POST(req) {
	const token = req.cookies.get('refresh_token');
	if (token) {
		const accessToken = await refreshAccessToken(token);
		if (accessToken.refresh_token) {
			return new Response(accessToken.refresh_token, { status: 200 });
		}
	}

	return new Response(null, { status: 401 });
}
