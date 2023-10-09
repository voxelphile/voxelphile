
import { error, fail, redirect } from "@sveltejs/kit";
import { fetch_promise } from "../../user-form.js";

/** @type {import('./$types').LayoutServerLoad} */
export async function load(event) {
    if (event.cookies.get("jwt") == undefined) {
        throw redirect(302, "/user/login");
    }

	const request = new Request("http://127.0.0.1:26541/user", {
        method: 'GET',
        headers: {
            'Authorization': 'Bearer ' + event.cookies.get("jwt")
        },
    });
    
    let response = await fetch_promise(request);

    let json = await response.json();

    console.log(json);

    if (json['profile'] == undefined) {
        return {};
    }
    
    json = { ...json,  profile_url: "https://storage.cloud.google.com/voxelphile/user/profile/" + json.profile + ".jpeg" };

    delete json['profile'];
    
    return json;
}