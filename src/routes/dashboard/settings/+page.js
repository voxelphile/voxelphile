/** @type {import('./$types').PageLoad} */
export async function load({ parent }) {
    let parent_json = await parent();
    console.log(parent_json);
	return {
        ...parent_json,

    };
}