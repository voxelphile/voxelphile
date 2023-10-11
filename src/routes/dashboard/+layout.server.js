
import { error, fail, redirect } from "@sveltejs/kit";
import { fetch_promise } from "../../user-form.js";
import { api } from "../../const.js";

/** @type {import('./$types').LayoutServerLoad} */
export async function load(event) {
    if (event.cookies.get("jwt") == undefined) {
        throw redirect(302, "/user/login");
    }

	const request = new Request(api + "/user", {
        method: 'GET',
        headers: {
            'Authorization': 'Bearer ' + event.cookies.get("jwt")
        },
    });
    
    let response = await fetch(request).catch((response) => {
        throw error(response?.status);
    });

    if (response?.status != 200) {
        throw error(response?.status);
    }

    let json = await response.json();

    console.log(json);

    if (json['profile'] != undefined) {
        json = { ...json,  profile_url: "https://storage.cloud.google.com/voxelphile/user/profile/" + json.profile + ".jpeg" };
    
        delete json['profile'];
    }
    
    return json;
}