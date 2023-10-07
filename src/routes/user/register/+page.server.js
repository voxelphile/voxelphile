/** @type {import('./$types').Actions} */
import {get_local_user_form_errors} from "../../../user-form.js"
export const actions = {
	default: async (event) => {
		const formData = await event.request.formData();
        const username = formData.get('username')?.toString();        
        const password = formData.get('password')?.toString();
        const email = formData.get('email')?.toString();        

        const errors = get_local_user_form_errors(formData);
        if(Object.keys(errors).length > 0) {
            return errors;
        }

        if (username == undefined || password == undefined || email == undefined) {
            return { success: false };
        }

        let json = { username, details: { password, email } };

        const request = new Request("https://api.voxelphile.com/user/register", {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(json),
        });

        fetch(request)
            .then((response) => {
                console.log(response);
            });
        
        return { success: true };
	}
}; 