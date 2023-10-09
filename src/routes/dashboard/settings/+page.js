/** @type {import('./$types').PageLoad} */
export async function load({ parent }) {
	return await parent();
}