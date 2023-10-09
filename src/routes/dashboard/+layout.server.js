
import { error, fail } from "@sveltejs/kit";
import { fetch_promise } from "../../user-form.js";


/** @type {import('./$types').LayoutServerLoad} */
export async function load() {
	const request = new Request("http://127.0.0.1:26541/user", {
        method: 'GET',
        headers: {
            'Authorization': 'Bearer eyJhbGciOiJIUzI1NiJ9.eyJpZCI6ImEwZTBhNzZkLTQ0NGItNGJhNi04NzU3LWNmYzg4NTA0M2VhNCIsImV4cCI6MTY5Njk1Mjk2MH0.rJffEGg_wzduk223chsxa-98P4uvc2Y1KxTqXRWpou4'
        },
    });
    
    let response = await fetch_promise(request);

    let json = await response.json();
    
    if (json['profile'] == undefined) {
        return {};
    }
    
    json = { ...json, profile_url: "https://storage.cloud.google.com/voxelphile/user/profile/" + json.profile + ".jpeg" };

    delete json['profile'];
    
    return json;
}
