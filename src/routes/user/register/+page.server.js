import { error, fail } from "@sveltejs/kit";
import { fetch_promise } from "../../../user-form.js";
/** @type {import('./$types').Actions} */
import { api } from "../../../const.js";
export const actions = {
	default: async (event) => {
		const formData = await event.request.formData();
        const username = formData.get('username')?.toString();        
        const password = formData.get('password')?.toString();
        const email = formData.get('email')?.toString();        

        if (username == undefined || password == undefined || email == undefined) {
            return { success: false };
        }

        let json = { username, details: { password, email } };

        const request = new Request(api + "/user/register", {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(json),
        });

        let response;
        try {
            response = await fetch_promise(request);
        } catch (err) {
            throw error(503, "Service unavailable");
        }
        
        return { success: true };
	}
}; 