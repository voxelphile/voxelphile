/** @type {import('./$types').PageLoad} */
export async function load({ parent }) {
    let parent_json = await parent();
    console.log("yo" + parent_json);
	return {
        ...parent_json,

    };
}