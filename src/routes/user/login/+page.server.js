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

        console.log("yo");

        let response = await fetch_promise(request);
       

        console.log(await response.text());
        
        return { success: true };
	}
};