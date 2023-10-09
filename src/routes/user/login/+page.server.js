import { error, fail } from "@sveltejs/kit";
import { fetch_promise } from "../../../user-form.js";
/** @type {import('./$types').Actions} */
import { api } from "../../../const.js";
export const actions = {
	default: async (event) => {
		const formData = await event.request.formData();
        const email = formData.get('email')?.toString();        
        const password = formData.get('password')?.toString();

        let json = { password, email };


        const request = new Request(api + "/user/login", {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(json),
        });
        let response = await fetch_promise(request);

        let jwt = JSON.parse(await response.text());
        
        event.cookies.set("jwt", jwt, { path: '/' });
        
        return { };
	}
};