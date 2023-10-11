/** @type {import('./$types').PageLoad} */
export async function load({ parent }) {
    let parent_json = await parent();
	return {
        ...parent_json,

    };
}