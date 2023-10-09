/** @type {import('./$types').Actions} */
import { api } from "../../../const.js";
import { fetch_promise } from "../../../user-form.js";


export const actions = {
	default: async (event) => {
		const formData = await event.request.formData();

        const profile = formData.get('profile')?.toString();        

        let json = { };

        if (profile != null && profile != '') {
            json['profile'] = profile;
        }
        console.log(json);
        const request = new Request(api + "/user/change", {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': 'Bearer eyJhbGciOiJIUzI1NiJ9.eyJpZCI6ImEwZTBhNzZkLTQ0NGItNGJhNi04NzU3LWNmYzg4NTA0M2VhNCIsImV4cCI6MTY5Njk1Mjk2MH0.rJffEGg_wzduk223chsxa-98P4uvc2Y1KxTqXRWpou4'
            },
            body: JSON.stringify(json),
        });
        
        let response = await fetch_promise(request);


        console.log(response);

        return { success: true };
	}
};