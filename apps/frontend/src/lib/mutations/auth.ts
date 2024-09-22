import { client } from '../client';

export async function refreshAccessToken(refreshToken: string) {
	return await client.mutation(['authentication.refresh_token', refreshToken]);
}
