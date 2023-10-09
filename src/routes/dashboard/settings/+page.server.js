/** @type {import('./$types').Actions} */
import { api } from "../../../const.js";
import { fetch_promise } from "../../../user-form.js";


export const actions = {
	default: async (event) => {
		const formData = await event.request.formData();    

        let json = { };

        if (formData.get('profile') != null && formData.get('profile')) {
            if (formData.get('profile') instanceof String) {
                json['profile'] = formData.get('profile').toString();
            }
        }

        if (formData.get('email') != null) {
            if (formData.get('email')?.toString() != '') {
                json['email'] = formData.get('email')?.toString();
            }
        }
        if (formData.get('username') != null ) {
            if (formData.get('username')?.toString() != '') {
                json['username'] = formData.get('username')?.toString();
            }
        }

        if (formData.get('password') != null && formData.get('password') == formData.get('repassword')
        ) {
            if (formData.get('password')?.toString() != '') {
                json['password'] = formData.get('password')?.toString();
            }
        }


        console.log(json);

        const request = new Request(api + "/user/change", {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': 'Bearer ' + event.cookies.get("jwt")
            },
            body: JSON.stringify(json),
        });
        
        let response = await fetch_promise(request);


        console.log(response);

        return { success: true };
	}
};